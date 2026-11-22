//! One shared package, with a durable independent identity on each driving PC.
use super::model::{new_id, AgentConfig};
use anyhow::{ensure, Context, Result};
use std::{io::Write, path::Path};

pub fn resolve(mut config: AgentConfig, local_root: &Path) -> Result<AgentConfig> {
    if !config.shared_package {
        return Ok(config);
    }
    ensure!(
        config.source_id.len() == 32 && config.source_id.chars().all(|c| c.is_ascii_hexdigit()),
        "Invalid shared package identity"
    );
    let root = local_root.join("identities");
    std::fs::create_dir_all(&root)?;
    let parent = config.source_id.clone();
    let guard = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join(format!("{parent}.lock")))?;
    guard.lock()?;
    let path = root.join(format!("{parent}.json"));
    match std::fs::read(&path) {
        Ok(bytes) => {
            let saved: AgentConfig = serde_json::from_slice(&bytes)
                .context("Saved sender identity is damaged; preserve it before re-pairing")?;
            ensure!(
                saved.enrollment_id.as_deref() == Some(&parent)
                    && saved.entry_id == config.entry_id
                    && saved.event_id == config.event_id
                    && saved.certificate_der == config.certificate_der
                    && saved.token == config.token
                    && !saved.shared_package,
                "Saved sender identity does not match this shared package"
            );
            ensure!(
                saved.source_id.len() == 32
                    && saved.source_id.chars().all(|c| c.is_ascii_hexdigit()),
                "Invalid saved sender identity"
            );
            Ok(saved)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            config.source_id = new_id();
            config.enrollment_id = Some(parent);
            config.shared_package = false;
            let pending = path.with_extension("pending");
            let mut file = std::fs::File::create(&pending)?;
            file.write_all(&serde_json::to_vec(&config)?)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(pending, path)?;
            Ok(config)
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_shared_package_creates_three_stable_pc_identities() {
        let config = AgentConfig {
            source_id: new_id(),
            shared_package: true,
            token: "shared-key".into(),
            entry_id: "Race car".into(),
            event_id: new_id(),
            certificate_der: "pinned-cert".into(),
            ..Default::default()
        };
        let mut ids = std::collections::BTreeSet::new();
        for _ in 0..3 {
            let pc = tempfile::tempdir().unwrap();
            let first = resolve(config.clone(), pc.path()).unwrap();
            let again = resolve(config.clone(), pc.path()).unwrap();
            assert_eq!(first.source_id, again.source_id);
            assert_eq!(
                first.enrollment_id.as_deref(),
                Some(config.source_id.as_str())
            );
            assert_eq!(first.entry_id, config.entry_id);
            ids.insert(first.source_id);
        }
        assert_eq!(ids.len(), 3);
    }
    #[test]
    fn damaged_identity_is_preserved_and_old_packages_are_unchanged() {
        let pc = tempfile::tempdir().unwrap();
        let mut config = AgentConfig {
            source_id: new_id(),
            ..Default::default()
        };
        assert_eq!(
            resolve(config.clone(), pc.path()).unwrap().source_id,
            config.source_id
        );
        assert!(!pc.path().join("identities").exists());
        config.shared_package = true;
        resolve(config.clone(), pc.path()).unwrap();
        let path = pc
            .path()
            .join("identities")
            .join(format!("{}.json", config.source_id));
        std::fs::write(&path, "damaged").unwrap();
        assert!(resolve(config, pc.path()).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"damaged");
    }
}
