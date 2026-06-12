//! P4 TDD targets (write these RED first, then implement the modules).
//! Fixture geometries: keep a couple of WKT points + a polygon inline.

#[test]
#[ignore = "P4: implement h3::latlng_to_cell"]
fn latlng_to_known_h3_cell() {
    // Bangkok ~ (13.7563, 100.5018) at res 7 → a known h3 index (fill in at impl).
    // let cell = mimir_geo::h3::latlng_to_cell(13.7563, 100.5018, 7).unwrap();
    // assert_eq!(cell, "87a...");
}

#[test]
#[ignore = "P4: implement spatial::geo_buffer"]
fn buffer_grows_area() {
    // buffer a point by 100 m → a polygon whose area > 0.
}

#[test]
#[ignore = "P4: implement engine spatial query (ST_*)"]
fn spatial_extension_loads_and_queries() {
    // GeoEngine::open() then SELECT ST_Point(0,0) IS NOT NULL.
}
