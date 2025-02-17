use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Type {
    InferDelegation(),
    Slice(Box<Type>),
    Array(Box<Type>),
    Ptr(Box<MutableType>),
    Ref(Box<MutableType>),
    BareFn(),
    Never,
    Tuple(Vec<Type>),
    Path(String),
    OpaqueDef,
    TraitAscription,
    TraitObject,
    Typeof(),
    Infer,
    Error,
    Pat,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "mutability", rename_all = "snake_case")]
pub enum MutableType {
    Mutable(Type),
    Immutable(Type),
}
