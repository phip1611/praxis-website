// SPDX-License-Identifier: MIT OR Apache-2.0

//! Structured data in JSON-LD for search engines.
//!
//! The graph keeps the `@id`s of the WordPress site (Yoast SEO), so search
//! engines recognize the entities. On top of that, the practice is a
//! schema.org `Physician` with address, opening hours, and contact data,
//! which local search results can show directly.

use crate::config::BaseUrl;
use crate::content::site::SITE;
use serde_json::{Value, json};

/// A page image as structured data.
#[derive(Debug)]
pub(crate) struct ImageFacts<'a> {
    /// The absolute URL.
    pub(crate) url: &'a str,
    /// The width in pixels.
    pub(crate) width: u32,
    /// The height in pixels.
    pub(crate) height: u32,
    /// The caption.
    pub(crate) caption: &'a str,
}

/// The facts about one page.
#[derive(Debug)]
pub(crate) struct PageFacts<'a> {
    /// The canonical URL.
    pub(crate) url: &'a str,
    /// The title.
    pub(crate) name: &'a str,
    /// The description.
    pub(crate) description: &'a str,
    /// The breadcrumb label, `None` for the start page.
    pub(crate) breadcrumb: Option<&'a str>,
    /// When the page last changed, if it's known.
    pub(crate) modified: Option<&'a str>,
    /// The main image, if any.
    pub(crate) image: Option<ImageFacts<'a>>,
}

/// Returns the JSON-LD graph of a page. The template embeds it with askama's
/// `json` filter, which escapes `<`, so the data can't end the `<script>`.
pub(crate) fn graph(base: &BaseUrl, logo: &ImageFacts<'_>, page: &PageFacts<'_>) -> Value {
    let home = base.join("/");
    let mut graph = vec![web_page(&home, page), breadcrumbs(&home, page)];
    if let Some(image) = &page.image {
        graph.push(json!({
            "@type": "ImageObject",
            "inLanguage": "de",
            "@id": format!("{}#primaryimage", page.url),
            "url": image.url,
            "contentUrl": image.url,
            "width": image.width,
            "height": image.height,
            "caption": image.caption,
        }));
    }
    graph.push(web_site(&home));
    graph.push(organization(&home, logo));
    json!({ "@context": "https://schema.org", "@graph": graph })
}

/// Returns the `WebPage` node.
fn web_page(home: &str, page: &PageFacts<'_>) -> Value {
    let mut node = json!({
        "@type": "WebPage",
        "@id": page.url,
        "url": page.url,
        "name": page.name,
        "description": page.description,
        "isPartOf": { "@id": format!("{home}#website") },
        "breadcrumb": { "@id": format!("{}#breadcrumb", page.url) },
        "inLanguage": "de",
        "potentialAction": [{ "@type": "ReadAction", "target": [page.url] }],
    });
    if let Some(modified) = page.modified {
        node["dateModified"] = json!(modified);
    }
    if page.breadcrumb.is_none() {
        node["about"] = json!({ "@id": format!("{home}#organization") });
    }
    if page.image.is_some() {
        let image = json!({ "@id": format!("{}#primaryimage", page.url) });
        node["primaryImageOfPage"] = image.clone();
        node["image"] = image;
    }
    node
}

/// Returns the `BreadcrumbList` node.
fn breadcrumbs(home: &str, page: &PageFacts<'_>) -> Value {
    let mut items = vec![json!({
        "@type": "ListItem",
        "position": 1,
        "name": "Home",
        "item": home,
    })];
    if let Some(label) = page.breadcrumb {
        items.push(json!({ "@type": "ListItem", "position": 2, "name": label }));
    }
    json!({
        "@type": "BreadcrumbList",
        "@id": format!("{}#breadcrumb", page.url),
        "itemListElement": items,
    })
}

/// Returns the `WebSite` node.
///
/// Unlike on the WordPress site, there is no `SearchAction`, as the site
/// has no search. Announcing one would lead visitors to empty results.
fn web_site(home: &str) -> Value {
    json!({
        "@type": "WebSite",
        "@id": format!("{home}#website"),
        "url": home,
        "name": SITE.practice.site_name,
        "description": SITE.practice.tagline,
        "publisher": { "@id": format!("{home}#organization") },
        "inLanguage": "de",
    })
}

/// Returns the node of the practice.
fn organization(home: &str, logo: &ImageFacts<'_>) -> Value {
    let logo_id = format!("{home}#/schema/logo/image/");
    let opening_hours: Vec<Value> = SITE
        .opening_hours
        .iter()
        .flat_map(|day| {
            day.ranges().map(|range| {
                json!({
                    "@type": "OpeningHoursSpecification",
                    "dayOfWeek": day.schema_day,
                    "opens": range.opens,
                    "closes": range.closes,
                })
            })
        })
        .collect();
    json!({
        "@type": "Physician",
        "@id": format!("{home}#organization"),
        "name": SITE.practice.site_name,
        "url": home,
        "logo": {
            "@type": "ImageObject",
            "inLanguage": "de",
            "@id": logo_id,
            "url": logo.url,
            "contentUrl": logo.url,
            "width": logo.width,
            "height": logo.height,
            "caption": logo.caption,
        },
        "image": { "@id": logo_id },
        "medicalSpecialty": "https://schema.org/PrimaryCare",
        "telephone": SITE.practice.phone.international,
        "faxNumber": SITE.practice.fax.international,
        "email": SITE.practice.email,
        "address": {
            "@type": "PostalAddress",
            "streetAddress": SITE.practice.street,
            "postalCode": SITE.practice.postal_code,
            "addressLocality": SITE.practice.locality,
            "addressRegion": SITE.practice.region,
            "addressCountry": "DE",
        },
        "geo": {
            "@type": "GeoCoordinates",
            "latitude": SITE.practice.latitude,
            "longitude": SITE.practice.longitude,
        },
        "hasMap": SITE.practice.maps_url,
        "openingHoursSpecification": opening_hours,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The facts of the contact page, which has all optional parts.
    fn contact_page() -> PageFacts<'static> {
        PageFacts {
            url: "https://praxis.test/kontakt/",
            name: "Kontakt – Hausarztpraxis Dirk Schuster",
            description: "Kontakt",
            breadcrumb: Some("Kontakt"),
            modified: Some("2015-09-04T22:38:53+00:00"),
            image: Some(ImageFacts {
                url: "https://praxis.test/card.png",
                width: 1800,
                height: 1200,
                caption: "Visitenkarte",
            }),
        }
    }

    /// The logo of the test site.
    const fn logo() -> ImageFacts<'static> {
        ImageFacts {
            url: "https://praxis.test/logo.png",
            width: 512,
            height: 512,
            caption: "Logo",
        }
    }

    /// Returns the graph of `page`.
    fn parsed(page: &PageFacts<'_>) -> Value {
        let base = BaseUrl::parse("https://praxis.test").expect("URL should be valid");
        graph(&base, &logo(), page)
    }

    /// Returns the node of `@type` in the graph.
    fn node<'a>(graph: &'a Value, kind: &str) -> &'a Value {
        graph["@graph"]
            .as_array()
            .expect("graph should be an array")
            .iter()
            .find(|node| node["@type"] == kind)
            .unwrap_or_else(|| panic!("graph should contain {kind}"))
    }

    /// The graph has all nodes and keeps the IDs of the WordPress site.
    #[test]
    fn graph_contains_all_nodes_with_stable_ids() {
        let graph = parsed(&contact_page());

        assert_eq!(
            node(&graph, "WebPage")["@id"],
            "https://praxis.test/kontakt/"
        );
        assert_eq!(
            node(&graph, "WebSite")["@id"],
            "https://praxis.test/#website"
        );
        assert_eq!(
            node(&graph, "Physician")["@id"],
            "https://praxis.test/#organization"
        );
        assert_eq!(node(&graph, "ImageObject")["width"], 1800);
        assert_eq!(
            node(&graph, "BreadcrumbList")["itemListElement"][1]["name"],
            "Kontakt"
        );
    }

    /// The opening hours match the table, one entry per time range.
    #[test]
    fn graph_contains_opening_hours() {
        let graph = parsed(&contact_page());
        let hours = node(&graph, "Physician")["openingHoursSpecification"]
            .as_array()
            .expect("hours should be an array")
            .clone();

        assert_eq!(hours.len(), 6);
        assert_eq!(hours[0]["dayOfWeek"], "https://schema.org/Monday");
        assert_eq!(hours[0]["opens"], "08:00");
        assert_eq!(hours[1]["opens"], "16:00");
    }

    /// The practice can be found by address and phone in local search.
    #[test]
    fn graph_contains_contact_data() {
        let graph = parsed(&contact_page());
        let practice = node(&graph, "Physician");

        assert_eq!(practice["address"]["postalCode"], "09380");
        assert_eq!(practice["telephone"], "+49372139040");
        assert!(practice["geo"]["latitude"].is_number());
    }

    /// The start page has no second breadcrumb and is about the practice.
    #[test]
    fn home_page_is_about_organization() {
        let home = PageFacts {
            url: "https://praxis.test/",
            breadcrumb: None,
            image: None,
            ..contact_page()
        };

        let graph = parsed(&home);

        let page = node(&graph, "WebPage");
        assert_eq!(page["about"]["@id"], "https://praxis.test/#organization");
        assert_eq!(
            node(&graph, "BreadcrumbList")["itemListElement"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
    }

    /// There is no search, so no `SearchAction` may be announced.
    #[test]
    fn web_site_has_no_search_action() {
        let graph = parsed(&contact_page());

        assert!(node(&graph, "WebSite").get("potentialAction").is_none());
    }
}
