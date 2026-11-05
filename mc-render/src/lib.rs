//! Renderização com wgpu (Vulkan no Linux).

pub mod anim;
mod arena;
mod assets;
pub mod container_tex;
pub mod items;
pub mod particle_pass;
pub mod resources;
mod renderer;
mod sections;
pub mod sky;
pub mod skygeo;
pub mod ui;

pub use assets::{block_tile, build_atlas, tile_size_for, AtlasImage, MAX_TILE};
pub use resources::Jar;
pub use sections::SectionSet;
pub use particle_pass::{ParticleBatch, ParticleDraw, ParticleInstance};
pub use renderer::{aabb_visible, Camera, DrawItem, FarFrame, Frame, GuiModels, HandPass, Renderer, Sky, Tex};
pub use skygeo::{weather_draws, CloudFrame, SkyDraw, SkyFrame, SkyTex, SkyVertex, WeatherFrame, WeatherWorld};
