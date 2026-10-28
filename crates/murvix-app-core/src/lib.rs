//! Headless client state, configuration, and persistence.
//!
//! Cached project views, tabs, panes, and window state live here; server
//! execution and UI toolkit code do not.

pub mod agent_launch;
pub mod agent_sessions;
pub mod backup;
pub mod browser;
pub mod composer;
mod error;
pub mod extensions;
mod groups;
mod home;
mod ids;
mod layout;
pub mod localization;
pub mod modal;
pub mod opener;
mod pane;
mod project;
pub mod project_layouts;
pub mod qr;
pub mod restore;
mod servers;
mod state;
pub mod store;
mod tab;
pub mod title;
pub mod webview;
mod window;

pub use error::AppError;
pub use groups::{TabGroup, TabGroups};
pub use ids::{GroupId, PaneId, ProjectId, ServerId, TabId};
pub use layout::{Axis, Branch, Direction, Layout};
pub use pane::{Pane, PaneContent};
pub use project::{Color, PROJECT_COLORS, Project, ProjectKind, ProjectStatus, initial};
pub use state::AppState;
pub use tab::{Tab, TabCloseScope, TabSide};
pub use window::{WindowBounds, WindowState};

mod catalog;

pub mod settings;

pub mod activity;
