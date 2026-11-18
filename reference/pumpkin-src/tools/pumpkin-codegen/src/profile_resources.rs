use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use serde_json::Value;

pub fn canonical_id(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

pub fn resource_path(data: &Path, registry: &str, id: &str) -> PathBuf {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    data.join(namespace)
        .join(registry)
        .join(format!("{path}.json"))
}

pub fn read_json(path: &Path) -> Value {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

pub fn files(data: &Path, registry: &str) -> io::Result<BTreeMap<String, PathBuf>> {
    fn collect(
        root: &Path,
        dir: &Path,
        namespace: &str,
        files: &mut BTreeMap<String, PathBuf>,
    ) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                collect(root, &path, namespace, files)?;
            } else if path.extension().is_some_and(|ext| ext == "json") {
                let name = path.strip_prefix(root).unwrap().with_extension("");
                files.insert(
                    format!("{namespace}:{}", name.to_string_lossy().replace('\\', "/")),
                    path,
                );
            }
        }
        Ok(())
    }

    let mut files = BTreeMap::new();
    for entry in fs::read_dir(data)? {
        let entry = entry?;
        let root = entry.path().join(registry);
        if root.is_dir() {
            collect(
                &root,
                &root,
                &entry.file_name().to_string_lossy(),
                &mut files,
            )?;
        }
    }
    Ok(files)
}

pub fn profile_noise_references(
    value: &mut Value,
    prefix: &str,
    noises: &BTreeMap<String, PathBuf>,
) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if matches!(key.as_str(), "noise" | "noise_id")
                    && let Some(id) = value.as_str()
                {
                    let id = canonical_id(id);
                    if noises.contains_key(&id) {
                        *value = Value::String(format!("{prefix}/{id}"));
                    }
                } else {
                    profile_noise_references(value, prefix, noises);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                profile_noise_references(value, prefix, noises);
            }
        }
        _ => {}
    }
}
