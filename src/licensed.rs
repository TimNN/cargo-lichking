use std::path::PathBuf;

use cargo::CargoResult;
use cargo::core::Package;

use license::License;

#[derive(Debug, Eq, PartialEq)]
pub enum Confidence {
    Confident,
    SemiConfident,
    Unsure,
}

pub struct LicenseText {
    pub path: PathBuf,
    pub text: String,
    pub confidence: Confidence,
}

pub trait Licensed {
    fn license(&self) -> License;
    fn license_text(&self, license: &License) -> CargoResult<Vec<LicenseText>>;
}

impl Licensed for Package {
    fn license(&self) -> License {
        let metadata = self.manifest().metadata();
        metadata.license
            .as_ref()
            .and_then(|license| license.parse::<License>().ok())
            .or_else(|| metadata.license_file
                     .as_ref()
                     .and_then(|file| self.root().join(file).canonicalize().ok())
                     .map(License::File))
            .unwrap_or_default()
    }

    fn license_text(&self, license: &License) -> CargoResult<Vec<LicenseText>> {
        Ok(vec![])
    }
}
