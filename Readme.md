
# Pre-requisites

- Build and run on Linux or WSL
- Latest [Rust](https://rust-lang.org/)
- Latest [Elm](https://elm-lang.org/)
- Terraform

# Build and run

Execute `./build-and-run.sh` to run the server locally. Note that push notificaions require a "secure context" and as such they, only work on localhost or with HTTPS.

# Deployment

The infrastructure can be created with `terraform apply`. Generally, this only needs to be executed once. Then as a first-time-setup, the `remote-create-liba-service.sh` must be run on the created EC2 server.

Then each time a modification is made, the applictaion must be built with `build.sh` or `./build-and-run.sh` and uploaded to the EC2 server with `remote-deploy-dist-folder.sh`.
