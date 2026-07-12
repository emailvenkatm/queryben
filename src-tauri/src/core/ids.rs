use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! newtype_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self { Self(Uuid::new_v4()) }
            pub fn as_uuid(&self) -> Uuid { self.0 }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl From<Uuid> for $name { fn from(u: Uuid) -> Self { Self(u) } }
        impl From<$name> for Uuid { fn from(n: $name) -> Self { n.0 } }
    };
}

newtype_id!(ConnectionId);
newtype_id!(AccountId);
newtype_id!(NotebookId);
newtype_id!(QueryId);
