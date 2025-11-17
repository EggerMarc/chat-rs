use serde::{Deserialize, Serialize};

use crate::core::messages::parts::Parts;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Content {
    pub parts: Parts,
    pub role: RoleEnum,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleEnum {
    #[default]
    User,
    System,
    Model,
}
