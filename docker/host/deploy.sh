#!/bin/sh

set -eux

# Create volumes because swarm cannot do it by itself
mkdir -p docker/volumes/db docker/volumes/shared

# Initialize local swarm
[ $(docker info --format '{{.Swarm.LocalNodeState}}') == "active" ] || docker swarm init

# Create shared network for services deployed to the swarm
docker network inspect localnet -f "Network exists" || docker network create localnet --scope swarm --driver overlay

# Pull all images
docker compose pull

# Deploy the stack
docker stack deploy --resolve-image never -c docker-compose.yml -d umi
