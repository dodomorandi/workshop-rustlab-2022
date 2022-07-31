use std::collections::HashSet;

use serde::Serialize;
use serde_with::skip_serializing_none;
use workshop_rustlab_2022::database::{Entry, GeoPoint2d, GeoShape, ServerField};

#[skip_serializing_none]
#[derive(Debug, Default, Serialize)]
pub struct PartialEntry<'a> {
    geo_point_2d: Option<&'a GeoPoint2d>,
    geo_shape: Option<&'a GeoShape>,
    name: Option<&'a str>,
    etichetta: Option<&'a str>,
    notetesto: Option<&'a str>,
    numeroantico: Option<&'a str>,
    numeromoderno: Option<&'a str>,
    link1: Option<&'a str>,
    link2: Option<&'a str>,
    link3: Option<&'a str>,
    piani: Option<&'a str>,
    arcate: Option<&'a str>,
    architravate: Option<&'a str>,
    architravate_con_colonne_di_legno: Option<&'a str>,
    archivolti: Option<&'a str>,
    modiglioni: Option<&'a str>,
    mensoloni_architravati: Option<&'a str>,
    stalla_e: Option<&'a str>,
    fienile_i: Option<&'a str>,
    rimessa_e: Option<&'a str>,
    scuderia_e: Option<&'a str>,
    attivita_commerciali_produttive_1: Option<&'a str>,
    attivita_commerciali_produttive_2: Option<&'a str>,
    attivita_commerciali_produttive_3: Option<&'a str>,
    attivita_commerciali_produttive_4: Option<&'a str>,
    attivita_commerciali_produttive_5: Option<&'a str>,
}

impl<'a> PartialEntry<'a> {
    pub fn from_entry_with_fields(entry: &'a Entry, fields: &HashSet<ServerField>) -> Self {
        use ServerField::*;

        let mut out = Self::default();

        macro_rules! field {
            ($name:ident) => {
                out.$name = Some(&entry.$name)
            };
        }

        for field in fields {
            match field {
                GeoPoint2d => field!(geo_point_2d),
                GeoShape => field!(geo_shape),
                Name => field!(name),
                Etichetta => field!(etichetta),
                Notetesto => field!(notetesto),
                Numeroantico => field!(numeroantico),
                Numeromoderno => field!(numeromoderno),
                Link1 => field!(link1),
                Link2 => field!(link2),
                Link3 => field!(link3),
                Piani => field!(piani),
                Arcate => field!(arcate),
                Architravate => field!(architravate),
                ArchitravateConColonneDiLegno => field!(architravate_con_colonne_di_legno),
                Archivolti => field!(archivolti),
                Modiglioni => field!(modiglioni),
                MensoloniArchitravati => field!(mensoloni_architravati),
                StallaE => field!(stalla_e),
                FienileI => field!(fienile_i),
                RimessaE => field!(rimessa_e),
                ScuderiaE => field!(scuderia_e),
                AttivitaCommercialiProduttive1 => field!(attivita_commerciali_produttive_1),
                AttivitaCommercialiProduttive2 => field!(attivita_commerciali_produttive_2),
                AttivitaCommercialiProduttive3 => field!(attivita_commerciali_produttive_3),
                AttivitaCommercialiProduttive4 => field!(attivita_commerciali_produttive_4),
                AttivitaCommercialiProduttive5 => field!(attivita_commerciali_produttive_5),
            }
        }

        out
    }
}

impl<'a> From<&'a Entry> for PartialEntry<'a> {
    fn from(entry: &'a Entry) -> Self {
        let Entry {
            geo_point_2d,
            geo_shape,
            name,
            etichetta,
            notetesto,
            numeroantico,
            numeromoderno,
            link1,
            link2,
            link3,
            piani,
            arcate,
            architravate,
            architravate_con_colonne_di_legno,
            archivolti,
            modiglioni,
            mensoloni_architravati,
            stalla_e,
            fienile_i,
            rimessa_e,
            scuderia_e,
            attivita_commerciali_produttive_1,
            attivita_commerciali_produttive_2,
            attivita_commerciali_produttive_3,
            attivita_commerciali_produttive_4,
            attivita_commerciali_produttive_5,
        } = entry;

        let geo_point_2d = Some(geo_point_2d);
        let geo_shape = Some(geo_shape);
        let name = Some(name.as_str());
        let etichetta = Some(etichetta.as_str());
        let notetesto = Some(notetesto.as_str());
        let numeroantico = Some(numeroantico.as_str());
        let numeromoderno = Some(numeromoderno.as_str());
        let link1 = Some(link1.as_str());
        let link2 = Some(link2.as_str());
        let link3 = Some(link3.as_str());
        let piani = Some(piani.as_str());
        let arcate = Some(arcate.as_str());
        let architravate = Some(architravate.as_str());
        let architravate_con_colonne_di_legno = Some(architravate_con_colonne_di_legno.as_str());
        let archivolti = Some(archivolti.as_str());
        let modiglioni = Some(modiglioni.as_str());
        let mensoloni_architravati = Some(mensoloni_architravati.as_str());
        let stalla_e = Some(stalla_e.as_str());
        let fienile_i = Some(fienile_i.as_str());
        let rimessa_e = Some(rimessa_e.as_str());
        let scuderia_e = Some(scuderia_e.as_str());
        let attivita_commerciali_produttive_1 = Some(attivita_commerciali_produttive_1.as_str());
        let attivita_commerciali_produttive_2 = Some(attivita_commerciali_produttive_2.as_str());
        let attivita_commerciali_produttive_3 = Some(attivita_commerciali_produttive_3.as_str());
        let attivita_commerciali_produttive_4 = Some(attivita_commerciali_produttive_4.as_str());
        let attivita_commerciali_produttive_5 = Some(attivita_commerciali_produttive_5.as_str());

        Self {
            geo_point_2d,
            geo_shape,
            name,
            etichetta,
            notetesto,
            numeroantico,
            numeromoderno,
            link1,
            link2,
            link3,
            piani,
            arcate,
            architravate,
            architravate_con_colonne_di_legno,
            archivolti,
            modiglioni,
            mensoloni_architravati,
            stalla_e,
            fienile_i,
            rimessa_e,
            scuderia_e,
            attivita_commerciali_produttive_1,
            attivita_commerciali_produttive_2,
            attivita_commerciali_produttive_3,
            attivita_commerciali_produttive_4,
            attivita_commerciali_produttive_5,
        }
    }
}
