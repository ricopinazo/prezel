use nixpacks::{
    create_docker_image,
    nixpacks::{builder::docker::DockerBuilderOptions, plan::generator::GeneratePlanOptions},
};

#[tokio::test]
async fn test_nixpacks() {
    create_docker_image(
        "examples/astro-drizzle",
        vec!["PORT=80"],
        &GeneratePlanOptions {
            plan: None,
            config_file: None,
        },
        &DockerBuilderOptions {
            name: Some("astro-drizzle-test".to_owned()),
            out_dir: None,
            print_dockerfile: false,
            tags: vec![],
            labels: vec![],
            quiet: false,
            cache_key: None,
            no_cache: true,
            inline_cache: false,
            cache_from: None,
            platform: vec![],
            current_dir: true,
            no_error_without_start: true,
            incremental_cache_image: None,
            cpu_quota: None, // TODO: use this to prevent a small machine from getting stuck
            memory: None,
            verbose: true,
            docker_host: Some("unix:///var/run/docker.sock".to_owned()),
            docker_tls_verify: None,
            docker_output: None,
            add_host: vec![],
            docker_cert_path: None,
        },
    )
    .await
    .unwrap();
}
