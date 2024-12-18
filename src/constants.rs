use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub const CONTENT_LENGTH_LIMIT: usize = 12 * 1024 * 1024;
pub const LISTEN_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 3000);
pub const PENDING_QUALITY: i32 = 80; // Quality setting for encoder for pending (uploaded) images

// Quality setting for encoder for rotating images
// Note that this was set to 100, as not to compromise on quality when (repeatedly)  rotating images
pub const ROTATION_QUALITY: i32 = 100;

// Image paths
pub const PENDING_PATH: [&str; 2] = ["data", "pending"]; // Uploaded but Review not yet submitted
pub const UNAPPROVED_PATH: [&str; 2] = ["data", "unapproved"]; // Submitted, but not yet approved
pub const ORIGINAL_PATH: [&str; 2] = ["data", "originals"]; // Approved "original" images (rotated and converted to AVIF)
pub const CACHE_PATH: [&str; 2] = ["data", "cache"]; // Cache for requests
pub const RAW_PATH: [&str; 2] = ["data", "raw"]; // Raw images as uploaded
