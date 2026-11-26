//! Documents on elements (Element documents REV00, owner: A on all five):
//! a data sheet, a spec, a maker's page, kept by where it is. An element
//! has its own, its type's, and the file's (kept from an IFC file read, see
//! [`crate::ifcdata`]). Information only: CreaBIM never reads inside one.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A document by where it is: a path (from the project's folder when it
/// is inside it, else in full) or a web address.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocRef {
    pub name: String,
    pub location: String,
}

impl DocRef {
    /// A web address rather than a file.
    pub fn is_link(&self) -> bool {
        is_link(&self.location)
    }

    /// A file picked on disk: named after the file, and kept by its path
    /// from `project_dir` when it is inside it (so it moves with the
    /// project), with `/` between folders.
    pub fn from_file(path: &Path, project_dir: Option<&Path>) -> DocRef {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let location = match project_dir.and_then(|d| path.strip_prefix(d).ok()) {
            Some(rel) => rel.to_string_lossy().replace('\\', "/"),
            None => path.to_string_lossy().into_owned(),
        };
        DocRef { name, location }
    }

    /// A web address typed in: named after its site.
    pub fn from_link(url: &str) -> DocRef {
        let url = url.trim();
        let url = if url.starts_with("www.") {
            format!("https://{url}")
        } else {
            url.to_string()
        };
        let site = url.split_once("://").map_or(url.as_str(), |x| x.1);
        let site = site
            .split('/')
            .next()
            .unwrap_or(site)
            .trim_start_matches("www.");
        DocRef {
            name: site.to_string(),
            location: url,
        }
    }

    /// Where to open it: a file's full path (a path from the project
    /// against `project_dir`), or the link itself.
    pub fn target(&self, project_dir: Option<&Path>) -> Target {
        if self.is_link() {
            return Target::Link(self.location.clone());
        }
        let p = PathBuf::from(&self.location);
        let p = match project_dir {
            Some(d) if p.is_relative() => d.join(p),
            _ => p,
        };
        Target::File(p)
    }
}

/// What opening a document opens.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    File(PathBuf),
    Link(String),
}

pub fn is_link(location: &str) -> bool {
    let l = location.trim().to_ascii_lowercase();
    l.starts_with("http://")
        || l.starts_with("https://")
        || l.starts_with("www.")
        || l.starts_with("mailto:")
}

/// Where a document of an element comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum From {
    Own,
    /// Its type's: changed on the type.
    Type,
    /// The file's, kept from an IFC file read: written back as it came.
    File,
}

/// Element `e`'s documents: its own, then its type's, then the file's.
pub fn of(model: &crate::Model, e: &crate::Element) -> Vec<(DocRef, From)> {
    let mut out: Vec<(DocRef, From)> = e
        .documents
        .iter()
        .cloned()
        .map(|d| (d, From::Own))
        .collect();
    let ty = e
        .of_type
        .as_ref()
        .and_then(|t| model.get(t.id))
        .filter(|t| matches!(t.kind, crate::ElementKind::ElementType(_)));
    if let Some(t) = ty {
        out.extend(t.documents.iter().cloned().map(|d| (d, From::Type)));
    }
    // the file's, on it or on its type
    let data = model.ifc_data();
    for g in std::iter::once(e.id.ifc_guid()).chain(ty.map(|t| t.id.ifc_guid())) {
        for d in data.documents_of(&g) {
            if !out.iter().any(|(x, _)| *x == d) {
                out.push((d, From::File));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_in_the_project_is_kept_by_its_path_from_it() {
        let dir = Path::new("C:/Jobs/House");
        let d = DocRef::from_file(Path::new("C:/Jobs/House/docs/casement-1200.pdf"), Some(dir));
        assert_eq!(
            d,
            DocRef {
                name: "casement-1200".into(),
                location: "docs/casement-1200.pdf".into()
            }
        );
        assert_eq!(
            d.target(Some(dir)),
            Target::File(dir.join("docs/casement-1200.pdf"))
        );
        let out = DocRef::from_file(Path::new("D:/Old/quote.pdf"), Some(dir));
        assert_eq!(out.location, "D:/Old/quote.pdf");
    }

    #[test]
    fn a_link_is_named_after_its_site() {
        let d = DocRef::from_link(" www.velux.com/products/ggl ");
        assert_eq!(d.name, "velux.com");
        assert_eq!(d.location, "https://www.velux.com/products/ggl");
        assert!(d.is_link());
        assert_eq!(d.target(None), Target::Link(d.location.clone()));
    }

    #[test]
    fn an_element_lists_its_own_then_its_types() {
        let mut m = crate::starter::starter_house();
        let w = m.iter().find(|e| e.of_type.is_some()).unwrap().clone();
        let ty = w.of_type.as_ref().unwrap().id;
        let mut t = m.get(ty).unwrap().clone();
        t.documents.push(DocRef {
            name: "Guide".into(),
            location: "docs/guide.pdf".into(),
        });
        let mut e = w.clone();
        e.documents
            .push(DocRef::from_link("https://example.com/a.pdf"));
        crate::Transaction::new("Docs")
            .replace(m.get(ty).unwrap().clone(), t)
            .replace(w, e.clone())
            .apply(&mut m)
            .unwrap();
        let got: Vec<_> = of(&m, m.get(e.id).unwrap())
            .into_iter()
            .map(|(d, f)| (d.name, f))
            .collect();
        assert_eq!(
            got,
            vec![
                ("example.com".to_string(), From::Own),
                ("Guide".to_string(), From::Type)
            ]
        );
    }
}
