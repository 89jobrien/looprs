generator target {
    output_type "rest/openapi"

    // Where the generated code will be saved (relative to baml_src/)
    output_dir "../generated"

    // Version of runtime to generate code for (should match installed `baml` package version)
    version "0.218.0"

    // 'baml-cli generate' will run this after generating openapi.yaml, to generate your OpenAPI client
    // This command will be run from within $output_dir
    // Note: Pin version for deterministic builds
    on_generate "npx @openapitools/openapi-generator-cli@7.20.0 generate -i openapi.yaml -g rust -o generated"
}
