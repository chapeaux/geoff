use camino::Utf8Path;
use rudof_lib::{
    RDFFormat, ReaderMode, Rudof, RudofConfig, ShaclFormat, ShaclValidationMode, ShapesGraphSource,
};

/// Result of SHACL validation against content data.
#[derive(Debug)]
pub struct ValidationOutcome {
    /// Whether all data conforms to the shapes.
    pub conforms: bool,
    /// Number of violations found.
    pub violations: usize,
    /// Number of warnings found.
    pub warnings: usize,
    /// Human-readable report text.
    pub report_text: String,
}

/// Validate RDF data (as Turtle) against a SHACL shapes file (also Turtle).
///
/// Both `data_ttl` and `shapes_ttl` are Turtle-formatted strings.
pub fn validate_shacl(
    data_ttl: &str,
    shapes_ttl: &str,
) -> std::result::Result<ValidationOutcome, Box<dyn std::error::Error>> {
    let config = RudofConfig::new().map_err(|e| format!("Failed to create rudof config: {e}"))?;
    let mut rudof =
        Rudof::new(&config).map_err(|e| format!("Failed to create rudof instance: {e}"))?;

    rudof
        .read_data(
            &mut data_ttl.as_bytes(),
            "content",
            Some(&RDFFormat::Turtle),
            None,
            Some(&ReaderMode::Lax),
            Some(false),
        )
        .map_err(|e| format!("Failed to read data: {e}"))?;

    rudof
        .read_shacl(
            &mut shapes_ttl.as_bytes(),
            "shapes",
            Some(&ShaclFormat::Turtle),
            None,
            Some(&ReaderMode::Lax),
        )
        .map_err(|e| format!("Failed to read shapes: {e}"))?;

    let report = rudof
        .validate_shacl(
            Some(&ShaclValidationMode::Native),
            Some(&ShapesGraphSource::CurrentSchema),
        )
        .map_err(|e| format!("SHACL validation failed: {e}"))?;

    let report_no_colors = report.clone().without_colors();
    let report_text = format!("{report_no_colors}");

    Ok(ValidationOutcome {
        conforms: report.conforms(),
        violations: report.count_violations(),
        warnings: report.count_warnings(),
        report_text,
    })
}

/// Validate content data from a Turtle file against a SHACL shapes file.
pub fn validate_files(
    data_path: &Utf8Path,
    shapes_path: &Utf8Path,
) -> std::result::Result<ValidationOutcome, Box<dyn std::error::Error>> {
    let data_ttl = std::fs::read_to_string(data_path)?;
    let shapes_ttl = std::fs::read_to_string(shapes_path)?;
    validate_shacl(&data_ttl, &shapes_ttl)
}

/// Generate a basic SHACL shapes graph from content metadata patterns.
///
/// Produces a Turtle string defining NodeShapes for the given content types.
pub fn generate_shapes(content_types: &[&str]) -> String {
    let mut ttl = String::from(
        r#"@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix schema: <https://schema.org/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix geoff-shapes: <urn:geoff:shapes:> .

"#,
    );

    for ct in content_types {
        let schema_type = match *ct {
            "Blog Post" | "BlogPosting" => "BlogPosting",
            "Article" => "Article",
            "Web Page" | "WebPage" => "WebPage",
            "How-To Guide" | "HowTo" => "HowTo",
            "FAQ Page" | "FAQPage" => "FAQPage",
            "Event" => "Event",
            _ => continue,
        };

        let shape_name = format!("{schema_type}Shape");
        ttl.push_str(&format!(
            r#"geoff-shapes:{shape_name} a sh:NodeShape ;
    sh:targetClass schema:{schema_type} ;
    sh:property [
        sh:path schema:name ;
        sh:minCount 1 ;
        sh:maxCount 1 ;
        sh:datatype xsd:string ;
        sh:message "Every {schema_type} must have exactly one name" ;
    ] .

"#
        ));
    }

    ttl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_data_conforms() {
        let data = r#"
            @prefix schema: <https://schema.org/> .
            @prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
            <urn:geoff:content:hello> a schema:BlogPosting ;
                schema:name "Hello World" .
        "#;

        let shapes = r#"
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix schema: <https://schema.org/> .
            @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

            <urn:geoff:shapes:BlogPostingShape> a sh:NodeShape ;
                sh:targetClass schema:BlogPosting ;
                sh:property [
                    sh:path schema:name ;
                    sh:minCount 1 ;
                    sh:maxCount 1 ;
                    sh:datatype xsd:string ;
                ] .
        "#;

        let result = validate_shacl(data, shapes).unwrap();
        assert!(
            result.conforms,
            "Valid data should conform: {}",
            result.report_text
        );
        assert_eq!(result.violations, 0);
    }

    #[test]
    fn missing_required_field_fails() {
        let data = r#"
            @prefix schema: <https://schema.org/> .
            <urn:geoff:content:hello> a schema:BlogPosting .
        "#;

        let shapes = r#"
            @prefix sh: <http://www.w3.org/ns/shacl#> .
            @prefix schema: <https://schema.org/> .
            @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

            <urn:geoff:shapes:BlogPostingShape> a sh:NodeShape ;
                sh:targetClass schema:BlogPosting ;
                sh:property [
                    sh:path schema:name ;
                    sh:minCount 1 ;
                    sh:datatype xsd:string ;
                ] .
        "#;

        let result = validate_shacl(data, shapes).unwrap();
        assert!(!result.conforms, "Missing name should not conform");
        assert!(result.violations > 0);
    }

    #[test]
    fn generate_shapes_produces_valid_turtle() {
        let shapes = generate_shapes(&["Blog Post", "Article"]);
        assert!(shapes.contains("BlogPostingShape"));
        assert!(shapes.contains("ArticleShape"));
        assert!(shapes.contains("sh:targetClass"));
    }

    #[test]
    fn generate_shapes_skips_unknown_types() {
        let shapes = generate_shapes(&["UnknownType"]);
        assert!(!shapes.contains("Shape"));
    }
}
