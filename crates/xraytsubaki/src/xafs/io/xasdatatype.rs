use serde::{Deserialize, Serialize};
use version::version;

use crate::xafs::xasgroup::XASGroup;
use crate::xafs::xasspectrum::XASSpectrum;

#[derive(Serialize, Deserialize, Default, Debug)]
pub enum XASDataType {
    #[default]
    XASGroup,
    // Currently the xas bson is implemented only for XASGroup. I am thinking that it should not be implemented for XASSpectrum.
    XASSpectrum,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct XASGroupFile {
    pub version: String,
    pub name: String,
    pub datatype: XASDataType,
    pub data: XASGroup,
}

impl XASGroupFile {
    pub fn new() -> XASGroupFile {
        XASGroupFile {
            version: version!().to_string(),
            name: String::new(),
            datatype: XASDataType::XASGroup,
            data: XASGroup::new(),
        }
    }
}
