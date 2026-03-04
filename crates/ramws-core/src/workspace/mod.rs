pub mod manifest;
pub mod resolver;

pub use manifest::{
    Manifest, WorkspaceRecord, ensure_manifest, list_workspace_records, load_manifest,
    manifest_path, remove_workspace_record, save_manifest, save_workspace_record,
};
pub use resolver::{render_layout, resolve_workspace_path};
