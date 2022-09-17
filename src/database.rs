#![warn(clippy::pedantic)]

//! Helper structures and functions to easily interact with the example database.

use std::{collections::HashSet, ops::Not};

use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};

/// A single entry of the database.
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

/// A geographic point with longitude and latitude.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct GeoPoint2d {
    /// The longitude.
    pub lon: f64,

    /// The latitude.
    pub lat: f64,
}

/// A geographic shape.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GeoShape {
    Feature(geo_shape::Feature),
}

pub mod geo_shape {
    //! The supported types of geographic shapes.

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

/// The representation of the HTTP query supported by the server.
///
/// This type is exposed in order to make both the server and eventual clients share the same kind
/// of query. This should simplify writing a working client.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerQuery {
    /// The fields to include in the request. Keep in mind that you would need to create a
    /// `CustomEntry` struct based on [`Entry`] containing only the specified `fields`.
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub fields: HashSet<ServerField>,

    /// The page requested.
    ///
    /// If omitted, the first page is implied.
    pub page: Option<usize>,

    /// The size of the page.
    ///
    /// If omitted, [`DEFAULT_PAGE_SIZE`] is implied.
    pub page_size: Option<u16>,
}

impl ServerQuery {
    /// A simple helper to create a [`Request`] instance using the current fields.
    ///
    /// [`Request`]: `reqwest::Request`
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn create_request(&self, port: Option<u16>) -> reqwest::Request {
        let mut url = Url::parse("http://localhost").unwrap();
        url.set_port(port).unwrap();
        let mut request = reqwest::Request::new(Method::GET, url);

        request.url_mut().set_query(Some(
            &serde_qs::to_string(self).expect("all fields should be valid"),
        ));

        request
    }
}

/// The possible fields for the query/response.
///
/// All the variants have a direct relationship with a fields in [`Entry`].
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

impl ServerField {
    /// Return the string representation of the field.
    #[must_use]
    pub fn to_str(&self) -> &'static str {
        match self {
            ServerField::GeoPoint2d => "geo_point_2d",
            ServerField::GeoShape => "geo_shape",
            ServerField::Name => "name",
            ServerField::Etichetta => "etichetta",
            ServerField::Notetesto => "notetesto",
            ServerField::Numeroantico => "numeroantico",
            ServerField::Numeromoderno => "numeromoderno",
            ServerField::Link1 => "link1",
            ServerField::Link2 => "link2",
            ServerField::Link3 => "link3",
            ServerField::Piani => "piani",
            ServerField::Arcate => "arcate",
            ServerField::Architravate => "architravate",
            ServerField::ArchitravateConColonneDiLegno => "architravate_con_colonne_di_legno",
            ServerField::Archivolti => "archivolti",
            ServerField::Modiglioni => "modiglioni",
            ServerField::MensoloniArchitravati => "mensoloni_architravati",
            ServerField::StallaE => "stalla_e",
            ServerField::FienileI => "fienile_i",
            ServerField::RimessaE => "rimessa_e",
            ServerField::ScuderiaE => "scuderia_e",
            ServerField::AttivitaCommercialiProduttive1 => "attivita_commerciali_produttive1",
            ServerField::AttivitaCommercialiProduttive2 => "attivita_commerciali_produttive2",
            ServerField::AttivitaCommercialiProduttive3 => "attivita_commerciali_produttive3",
            ServerField::AttivitaCommercialiProduttive4 => "attivita_commerciali_produttive4",
            ServerField::AttivitaCommercialiProduttive5 => "attivita_commerciali_produttive5",
        }
    }
}

/// The number of fields in [`Entry`].
pub const FIELDS_LEN: u8 = 26;

/// The default page size for a request.
///
/// See [`ServerQuery::page_size`].
pub const DEFAULT_PAGE_SIZE: u16 = 10;

/// The default max bucket capacity.
pub const MAX_BUCKET_CAPACITY: u16 = 500;

/// The default leaky bucket leak-per-second.
pub const LEAK_PER_SECOND: u8 = 4;

/// Calculate the cost of a given query.
///
/// This is useful to evaluate if a [`LeakyBucket`] has enough free capacity to handle a specific
/// request.
///
/// [`LeakyBucket`]: crate::leaky_bucket::LeakyBucket
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
