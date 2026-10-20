use super::*;

#[test]
fn source_text_is_embedded_in_html_and_links_only_to_generated_pages() {
    let mut input = cone("a", vec![item("box", "Box", ApiKind::Class, None)]);
    input.sources[0].display_name = "file:///original-project/src/用户.scoop".into();
    let inputs = [input];
    let catalog = SiteCatalog::new(&inputs, SiteOptions::default()).unwrap();
    let files = render(&catalog);
    let source_url = routes::source(&inputs[0].key, &inputs[0].sources[0].key);
    let source = &files[&source_url];
    assert!(source.contains("public class Box"));
    assert!(source.contains("&lt;script&gt;"));
    assert!(!source.contains("<script>alert"));
    assert_target_exists(&files, &format!("{source_url}#L1"));
    let page = &catalog.routes[&SymbolKey("box".into())].page;
    let href = routes::relative(page, &format!("{source_url}#L1")).replace('/', "&#x2f;");
    assert!(files[page].contains(&format!("class=\"source-link\" href=\"{href}\"")));
    for html in files.values() {
        assert!(!html.contains("file:"));
        assert!(!html.contains("original-project"));
    }
}

#[test]
fn unavailable_source_omits_its_page_link_and_empty_source_listing() {
    let mut absent = item("absent", "Absent", ApiKind::Class, None);
    absent.location.source = SourceKey("unavailable".into());
    absent.comment = Some(comment("The API documentation remains available."));
    let mut input = cone("a", vec![item("box", "Box", ApiKind::Class, None), absent]);
    for with_source in [true, false] {
        if !with_source {
            input.sources.clear();
        }
        let inputs = [input.clone()];
        let catalog = SiteCatalog::new(&inputs, SiteOptions::default()).unwrap();
        let files = render(&catalog);
        let page = &catalog.routes[&SymbolKey("absent".into())].page;
        assert!(files[page].contains("The API documentation remains available."));
        assert!(!files[page].contains("class=\"source-link\""));
        assert!(!files.contains_key(&routes::source(
            &input.key,
            &SourceKey("unavailable".into())
        )));
        let page = &catalog.routes[&SymbolKey("box".into())].page;
        assert_eq!(files[page].contains("class=\"source-link\""), with_source);
        assert_eq!(
            files[&routes::cone(&input.key)].contains("Source files"),
            with_source
        );
        assert_eq!(
            files.contains_key(&routes::source(&input.key, &SourceKey("source".into()))),
            with_source
        );
    }
}
