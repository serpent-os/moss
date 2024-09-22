use filetime::FileTime;
use itertools::Itertools;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::{
    os::unix::fs::symlink,
    path::{Component, PathBuf},
    process::{Command, Stdio},
};

use fs_err as fs;
use moss::{dependency, Dependency, Provider};

use crate::package::collect::PathInfo;

use mailparse::{parse_mail, MailHeaderMap};

pub use self::elf::elf;
use super::{BoxError, BucketMut, Decision, Response};

mod elf;

pub fn include_any(_bucket: &mut BucketMut, _info: &mut PathInfo) -> Result<Response, BoxError> {
    Ok(Decision::IncludeFile.into())
}

pub fn ignore_blocked(_bucket: &mut BucketMut, info: &mut PathInfo) -> Result<Response, BoxError> {
    // non-/usr = bad
    if !info.target_path.starts_with("/usr") {
        return Ok(Decision::IgnoreFile {
            reason: "non /usr/ file".into(),
        }
        .into());
    }

    // libtool files break the world
    if info.file_name().ends_with(".la")
        && (info.target_path.starts_with("/usr/lib") || info.target_path.starts_with("/usr/lib32"))
    {
        return Ok(Decision::IgnoreFile {
            reason: "libtool file".into(),
        }
        .into());
    }

    Ok(Decision::NextHandler.into())
}

pub fn binary(bucket: &mut BucketMut, info: &mut PathInfo) -> Result<Response, BoxError> {
    if info.target_path.starts_with("/usr/bin") {
        let provider = Provider {
            kind: dependency::Kind::Binary,
            name: info.file_name().to_string(),
        };
        bucket.providers.insert(provider);
    } else if info.target_path.starts_with("/usr/sbin") {
        let provider = Provider {
            kind: dependency::Kind::SystemBinary,
            name: info.file_name().to_string(),
        };
        bucket.providers.insert(provider);
    }

    Ok(Decision::NextHandler.into())
}

pub fn pkg_config(bucket: &mut BucketMut, info: &mut PathInfo) -> Result<Response, BoxError> {
    let file_name = info.file_name();

    if !info.has_component("pkgconfig") || !file_name.ends_with(".pc") {
        return Ok(Decision::NextHandler.into());
    }

    let provider_name = file_name.strip_suffix(".pc").expect("extension exists");
    let emul32 = info.has_component("lib32");

    let provider = Provider {
        kind: if emul32 {
            dependency::Kind::PkgConfig32
        } else {
            dependency::Kind::PkgConfig
        },
        name: provider_name.to_string(),
    };

    bucket.providers.insert(provider);

    let output = Command::new("/usr/bin/pkg-config")
        .args(["--print-requires", "--print-requires-private", "--silence-errors"])
        .arg(&info.path)
        .envs([
            ("LC_ALL", "C"),
            (
                "PKG_CONFIG_PATH",
                if emul32 {
                    "/usr/lib32/pkgconfig:/usr/lib/pkgconfig:/usr/share/pkgconfig"
                } else {
                    "/usr/lib/pkgconfig:/usr/share/pkgconfig"
                },
            ),
        ])
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let deps = stdout.lines().filter_map(|line| line.split_whitespace().next());

    for dep in deps {
        let emul32_path = PathBuf::from(format!("/usr/lib32/pkgconfig/{dep}.pc"));
        let local_path = info
            .path
            .parent()
            .map(|p| p.join(format!("{dep}.pc")))
            .unwrap_or_default();

        let kind = if emul32 && (local_path.exists() || emul32_path.exists()) {
            dependency::Kind::PkgConfig32
        } else {
            dependency::Kind::PkgConfig
        };

        bucket.dependencies.insert(Dependency {
            kind,
            name: dep.to_string(),
        });
    }

    Ok(Decision::NextHandler.into())
}

pub fn python(bucket: &mut BucketMut, info: &mut PathInfo) -> Result<Response, BoxError> {
    let file_path = info.path.clone().into_os_string().into_string().unwrap_or_default();
    let is_dist_info = file_path.contains(".dist-info") && info.file_name().ends_with("METADATA");
    let is_egg_info = file_path.contains(".egg-info") && info.file_name().ends_with("PKG-INFO");

    if !(is_dist_info || is_egg_info) {
        return Ok(Decision::NextHandler.into());
    }

    let data = fs::read(&info.path)?;
    let mail = parse_mail(&data)?;
    let python_name = mail
        .get_headers()
        .get_first_value("Name")
        .unwrap_or_else(|| panic!("Failed to parse {}", info.file_name()));

    /* Insert generic provider */
    bucket.providers.insert(Provider {
        kind: dependency::Kind::Python,
        name: python_name.to_string(),
    });

    let output = Command::new("/usr/bin/python3")
        .arg("-c")
        .arg("import platform; print(f'{platform.python_version_tuple()[0]}.{platform.python_version_tuple()[1]}')")
        .stdout(Stdio::piped())
        .output()?;
    let python_version = String::from_utf8_lossy(&output.stdout);

    /* Insert versioned provider for auto deps */
    bucket.providers.insert(Provider {
        kind: dependency::Kind::Python,
        name: format!("{}({})", python_name, &python_version.to_string().trim_end()),
    });

    /* Now parse dependencies */
    let dist_path = info
        .path
        .parent()
        .unwrap_or_else(|| panic!("Failed to get parent path for {}", info.file_name()));
    let find_deps_script = include_str!("scripts/get-py-deps.py");

    let output = Command::new("/usr/bin/python3")
        .arg("-c")
        .arg(find_deps_script)
        .arg(dist_path)
        .stdout(Stdio::piped())
        .output()?;

    let deps = String::from_utf8_lossy(&output.stdout);
    for dep in deps.lines() {
        bucket.dependencies.insert(Dependency {
            kind: dependency::Kind::Python,
            name: format!("{}({})", &dep, &python_version.to_string().trim_end()),
        });
    }

    Ok(Decision::NextHandler.into())
}

pub fn cmake(bucket: &mut BucketMut, info: &mut PathInfo) -> Result<Response, BoxError> {
    let file_name = info.file_name();

    if (!file_name.ends_with("Config.cmake") && !file_name.ends_with("-config.cmake"))
        || file_name.ends_with("-Config.cmake")
    {
        return Ok(Decision::NextHandler.into());
    }

    let provider_name = file_name
        .strip_suffix("Config.cmake")
        .or_else(|| file_name.strip_suffix("-config.cmake"))
        .expect("extension exists");

    bucket.providers.insert(Provider {
        kind: dependency::Kind::CMake,
        name: provider_name.to_string(),
    });

    Ok(Decision::NextHandler.into())
}

pub fn compressman(bucket: &mut BucketMut, info: &mut PathInfo) -> Result<Response, BoxError> {
    if !bucket.recipe.parsed.options.compressman {
        return Ok(Decision::NextHandler.into());
    }

    let is_man_file = info.path.components().contains(&Component::Normal("man".as_ref()))
        && info.file_name().ends_with(|c| ('1'..'9').contains(&c));
    let is_info_file =
        info.path.components().contains(&Component::Normal("info".as_ref())) && info.file_name().ends_with(".info");

    if !(is_man_file || is_info_file) {
        return Ok(Decision::NextHandler.into());
    }

    let mut generated_path = PathBuf::new();

    let metadata = fs::metadata(&info.path)?;
    let atime = metadata.accessed()?;
    let mtime = metadata.modified()?;

    /* If we have a man/info symlink update the link to the compressed file */
    if info.path.is_symlink() {
        let new_original = format!("{}.zst", fs::canonicalize(&info.path)?.display());
        let new_link = format!("{}.zst", &info.path.display());

        /*
         * Depending on the order the files get analysed the new compressed file may not yet exist,
         * compress it _now_ so the correct metadata src info is returned to the binary writer.
         */
        if !std::path::Path::new(&new_original).exists() {
            let compressed_file = compress_file_zstd(fs::canonicalize(&info.path)?)?;
            let _ = bucket.paths.install().guest.join(compressed_file);
        }

        symlink(format!("{}.zst", fs::read_link(&info.path)?.display()), &new_link)?;

        /* Restore the original {a,m}times for reproducibility */
        filetime::set_symlink_file_times(
            &new_link,
            FileTime::from_system_time(atime),
            FileTime::from_system_time(mtime),
        )?;

        generated_path.push(bucket.paths.install().guest.join(new_link));
        return Ok(Decision::ReplaceFile {
            newpath: generated_path,
        }
        .into());
    }

    let mut compressed_file = PathBuf::from(format!("{}.zst", info.path.display()));

    /* We may have already compressed the file if we encountered a symlink to this file first */
    if !&compressed_file.exists() {
        compressed_file = compress_file_zstd(info.path.clone())?;
    }

    /* Restore the original {a,m}times for reproducibility */
    filetime::set_file_handle_times(
        &File::open(&compressed_file)?,
        Some(FileTime::from_system_time(atime)),
        Some(FileTime::from_system_time(mtime)),
    )?;

    generated_path.push(bucket.paths.install().guest.join(compressed_file));

    pub fn compress_file_zstd(path: PathBuf) -> Result<PathBuf, BoxError> {
        let output_path = PathBuf::from(format!("{}.zst", path.display()));
        let input = File::create(&output_path)?;
        let mut reader = BufReader::new(File::open(&path)?);
        let mut writer = BufWriter::new(input);

        zstd::stream::copy_encode(&mut reader, &mut writer, 16)?;

        writer.flush()?;

        Ok(output_path)
    }

    Ok(Decision::ReplaceFile {
        newpath: generated_path,
    }
    .into())
}
