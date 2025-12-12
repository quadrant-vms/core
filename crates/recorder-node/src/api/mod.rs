mod routes;

pub use routes::{
    get_thumbnail, get_thumbnail_grid, healthz, list_recordings, start_recording, stop_recording,
};
