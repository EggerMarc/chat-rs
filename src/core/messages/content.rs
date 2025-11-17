use crate::core::messages::parts::Parts;

#[derive(Clone, Debug, Default)]
pub struct Content {
    pub parts: Parts,
    pub role: RoleEnum,
}

#[derive(Default, Debug, Clone)]
pub enum RoleEnum {
    #[default]
    User,
    System,
    Model,
}
