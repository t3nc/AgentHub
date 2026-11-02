pub mod strategy;

pub use strategy::{
    LinkStrategy, copy_tree, create_dir_link, hardlink_tree, link_state, remove_linked_or_dir,
};
