use super::super::prelude as asset;
use crate::asset2::loaders::MetaDataStreamable;
use crate::asset2::metadata_location::MetaDataLocation;
use crate::prelude as dare;
use crate::render2::util::{handle_cast_stream, ElementFormat};
use bytemuck::Pod;
use derivative::Derivative;
use futures::{FutureExt, StreamExt, TryStreamExt};
use futures_core::stream::BoxStream;
use image::{EncodableLayout, GenericImageView};
use std::sync::Arc;

pub struct Image {}
impl asset::Asset for Image {
    type Metadata = ImageMetaData;
    type Loaded = ImageAsset;
}

#[derive(Debug, PartialEq)]
pub struct ImageAsset {
    pub image: image::DynamicImage,
}
impl Eq for ImageAsset {}

impl asset::AssetLoaded for ImageAsset {}

#[derive(Derivative, Debug, PartialEq, Clone)]
#[derivative(Hash)]
pub struct ImageMetaData {
    /// Location of the image
    pub location: MetaDataLocation,
    /// Name
    #[derivative(Hash = "ignore")]
    pub name: String,
}
unsafe impl Send for ImageMetaData {}
impl Unpin for ImageMetaData {}
impl Eq for ImageMetaData {}
impl asset::AssetMetadata for ImageMetaData {}

impl asset::loaders::MetaDataLoad for ImageMetaData {
    type Loaded = ImageAsset;
    type LoadInfo<'a>
    where
        Self: 'a,
    = ();

    async fn load<'a>(&self, load_info: Self::LoadInfo<'a>) -> anyhow::Result<Self::Loaded> {
        let bytes: Vec<u8> = match &self.location {
            MetaDataLocation::Url(url) => reqwest::get(url).await?.bytes().await?.to_vec(),
            MetaDataLocation::FilePath(path) => tokio::fs::read(path).await?.as_bytes().to_vec(),
            MetaDataLocation::Memory(mem) => unimplemented!(),
        };
        let image = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
        let image = image.decode()?;
        Ok(ImageAsset { image })
    }
}
