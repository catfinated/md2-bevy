//! PiCture eXchange image format asset loading
use bevy::asset::{AssetLoader, LoadContext, RenderAssetUsages};
use bevy::prelude::*;
use thiserror::Error;

/// Custom error type for PCX loading
#[derive(Debug, Error)]
pub enum PcxLoaderError {
    #[error("Failed to read PCX file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid PCX format: {0}")]
    InvalidFormat(String),
}

/// The PCX asset loader
#[derive(Default)]
pub struct PcxLoader;

const HDR_BYTES: usize = 128;

impl AssetLoader for PcxLoader {
    type Asset = Image;
    type Settings = ();
    type Error = PcxLoaderError;

    async fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let image_data = parse_pcx(&bytes)?;
        Ok(image_data)
    }

    fn extensions(&self) -> &[&str] {
        &["pcx"]
    }
}

/// Parse PCX data and convert to Bevy Image
fn parse_pcx(data: &[u8]) -> Result<Image, PcxLoaderError> {
    if data.len() < HDR_BYTES {
        return Err(PcxLoaderError::InvalidFormat(
            "File too small to be valid PCX".to_string(),
        ));
    }

    let manufacturer = data[0];
    if manufacturer != 0x0A {
        return Err(PcxLoaderError::InvalidFormat(
            "Not a valid PCX file".to_string(),
        ));
    }

    // Extract dimensions from header
    let xmin = u16::from_le_bytes([data[4], data[5]]) as u32;
    let ymin = u16::from_le_bytes([data[6], data[7]]) as u32;
    let xmax = u16::from_le_bytes([data[8], data[9]]) as u32;
    let ymax = u16::from_le_bytes([data[10], data[11]]) as u32;

    let width = xmax - xmin + 1;
    let height = ymax - ymin + 1;
    let mut rgba_data = vec![0u8; (width * height * 4) as usize];

    decode_pcx_data(data, &mut rgba_data, width, height)?;

    Ok(Image::new(
        bevy::render::render_resource::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        rgba_data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    ))
}

/// Decode PCX pixel data
fn decode_pcx_data(
    data: &[u8],
    output: &mut [u8],
    width: u32,
    height: u32,
) -> Result<(), PcxLoaderError> {
    if data.len() < HDR_BYTES {
        return Err(PcxLoaderError::InvalidFormat(
            "Header too small".to_string(),
        ));
    }

    let encoding = data[2];
    let bits_per_pixel = data[3];
    let planes = data[65] as u32;
    let bytes_per_line = u16::from_le_bytes([data[66], data[67]]) as usize;

    // Verify RLE encoding
    if encoding != 1 {
        return Err(PcxLoaderError::InvalidFormat(
            "Only RLE encoding is supported".to_string(),
        ));
    }

    // Decode based on bit depth and planes
    match (bits_per_pixel, planes) {
        (8, 1) => {
            // 8-bit indexed color
            decode_8bit_indexed(data, output, width, height, bytes_per_line)
        }
        (8, 3) | (8, 4) => {
            // 24-bit or 32-bit RGB
            decode_24bit_rgb(data, output, width, height, bytes_per_line, planes)
        }
        _ => Err(PcxLoaderError::InvalidFormat(format!(
            "Unsupported PCX format: {} bpp, {} planes",
            bits_per_pixel, planes
        ))),
    }
}

/// Decode 8-bit indexed PCX with 256-color palette
fn decode_8bit_indexed(
    data: &[u8],
    output: &mut [u8],
    width: u32,
    height: u32,
    bytes_per_line: usize,
) -> Result<(), PcxLoaderError> {
    // Extract palette from end of file (last 768 bytes after marker 0x0C)
    let palette_offset = data
        .len()
        .checked_sub(769)
        .ok_or_else(|| PcxLoaderError::InvalidFormat("File too small for palette".to_string()))?;

    if data[palette_offset] != 0x0C {
        return Err(PcxLoaderError::InvalidFormat(
            "Invalid palette marker".to_string(),
        ));
    }

    let palette = &data[palette_offset + 1..];
    let compressed = &data[HDR_BYTES..palette_offset];
    let total_bytes = bytes_per_line * height as usize;
    let decompressed = decompress_rle_data(compressed, total_bytes)?;

    // Convert indexed data to RGBA
    for y in 0..height as usize {
        for x in 0..width as usize {
            let src_idx = y * bytes_per_line + x;
            if src_idx >= decompressed.len() {
                return Err(PcxLoaderError::InvalidFormat(
                    "Insufficient data".to_string(),
                ));
            }

            let palette_idx = decompressed[src_idx] as usize * 3;
            if palette_idx + 2 >= palette.len() {
                return Err(PcxLoaderError::InvalidFormat(
                    "Invalid palette index".to_string(),
                ));
            }

            let dst_idx = (y * width as usize + x) * 4;
            output[dst_idx] = palette[palette_idx]; // R
            output[dst_idx + 1] = palette[palette_idx + 1]; // G
            output[dst_idx + 2] = palette[palette_idx + 2]; // B
            output[dst_idx + 3] = 255; // A
        }
    }

    Ok(())
}

/// Decode 24-bit RGB PCX
fn decode_24bit_rgb(
    data: &[u8],
    output: &mut [u8],
    width: u32,
    height: u32,
    bytes_per_line: usize,
    planes: u32,
) -> Result<(), PcxLoaderError> {
    let total_bytes = bytes_per_line * planes as usize * height as usize;
    let decompressed = decompress_rle_data(&data[HDR_BYTES..], total_bytes)?;

    // Convert planar RGB to interleaved RGBA
    for y in 0..height as usize {
        let scanline_offset = y * bytes_per_line * planes as usize;

        for x in 0..width as usize {
            let dst_idx = (y * width as usize + x) * 4;

            // RGB planes are stored sequentially in each scanline
            let r_offset = scanline_offset + x;
            let g_offset = scanline_offset + bytes_per_line + x;
            let b_offset = scanline_offset + bytes_per_line * 2 + x;

            if b_offset >= decompressed.len() {
                return Err(PcxLoaderError::InvalidFormat(
                    "Insufficient RGB data".to_string(),
                ));
            }

            output[dst_idx] = decompressed[r_offset]; // R
            output[dst_idx + 1] = decompressed[g_offset]; // G
            output[dst_idx + 2] = decompressed[b_offset]; // B
            output[dst_idx + 3] = 255; // A
        }
    }

    Ok(())
}

fn decompress_rle_data(data: &[u8], total_bytes: usize) -> Result<Vec<u8>, PcxLoaderError> {
    let mut decompressed = Vec::new();
    let mut i = 0;

    while i < data.len() && decompressed.len() < total_bytes {
        let byte = data[i];
        i += 1;

        if byte >= 0xC0 {
            // RLE run
            let count = (byte & 0x3F) as usize;
            if i >= data.len() {
                return Err(PcxLoaderError::InvalidFormat(
                    "Unexpected end of data".to_string(),
                ));
            }
            let value = data[i];
            i += 1;
            decompressed.extend(std::iter::repeat_n(value, count));
        } else {
            // Literal byte
            decompressed.push(byte);
        }
    }

    Ok(decompressed)
}

/// Plugin to register the PCX loader
pub struct PcxLoaderPlugin;

impl Plugin for PcxLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.register_asset_loader(PcxLoader);
    }
}
