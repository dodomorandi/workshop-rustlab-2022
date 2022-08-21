#![warn(clippy::pedantic)]

use std::{collections::HashSet, ops::Not};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Entry {
    pub geo_point_2d: GeoPoint2d,
    pub geo_shape: GeoShape,
    pub name: String,
    pub etichetta: String,
    pub notetesto: String,
    pub numeroantico: String,
    pub numeromoderno: String,
    pub link1: String,
    pub link2: String,
    pub link3: String,
    pub piani: String,
    pub arcate: String,
    pub architravate: String,
    pub architravate_con_colonne_di_legno: String,
    pub archivolti: String,
    pub modiglioni: String,
    pub mensoloni_architravati: String,
    pub stalla_e: String,
    pub fienile_i: String,
    pub rimessa_e: String,
    pub scuderia_e: String,
    pub attivita_commerciali_produttive_1: String,
    pub attivita_commerciali_produttive_2: String,
    pub attivita_commerciali_produttive_3: String,
    pub attivita_commerciali_produttive_4: String,
    pub attivita_commerciali_produttive_5: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct GeoPoint2d {
    pub lon: f64,
    pub lat: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GeoShape {
    Feature(geo_shape::Feature),
}

pub mod geo_shape {
    use super::{Deserialize, Serialize};

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct Feature {
        pub geometry: FeatureGeometry,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(tag = "type")]
    pub enum FeatureGeometry {
        Polygon { coordinates: Vec<Vec<[f64; 3]>> },
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerQuery {
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub fields: HashSet<ServerField>,
    pub page: Option<usize>,
    pub page_size: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerField {
    GeoPoint2d,
    GeoShape,
    Name,
    Etichetta,
    Notetesto,
    Numeroantico,
    Numeromoderno,
    Link1,
    Link2,
    Link3,
    Piani,
    Arcate,
    Architravate,
    ArchitravateConColonneDiLegno,
    Archivolti,
    Modiglioni,
    MensoloniArchitravati,
    StallaE,
    FienileI,
    RimessaE,
    ScuderiaE,
    AttivitaCommercialiProduttive1,
    AttivitaCommercialiProduttive2,
    AttivitaCommercialiProduttive3,
    AttivitaCommercialiProduttive4,
    AttivitaCommercialiProduttive5,
}

pub const FIELDS_LEN: u8 = 26;
pub const DEFAULT_PAGE_SIZE: u16 = 10;
pub const MAX_BUCKET_CAPACITY: u16 = 500;
pub const LEAK_PER_SECOND: u8 = 4;

#[must_use]
pub fn calc_query_cost(query: &ServerQuery) -> u16 {
    let fields_cost = query
        .fields
        .is_empty()
        .not()
        .then_some(query.fields.len())
        .unwrap_or_else(|| FIELDS_LEN.into())
        .try_into()
        .unwrap_or(u16::MAX);

    query
        .page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .saturating_mul(fields_cost)
}
