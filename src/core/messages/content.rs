use crate::core::messages::parts::Parts;

#[derive(Clone, Debug, Default)]
pub struct Content {
    pub parts: Parts,
    pub role: RoleEnum,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum RoleEnum {
    #[default]
    User,
    System,
    Model,
}
