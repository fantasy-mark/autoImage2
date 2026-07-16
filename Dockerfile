# The node alpine image is available here: https://github.com/nodejs/docker-node
FROM --platform=linux/amd64 node:24-alpine AS alpine

# It's important to update the index before installing packages to ensure you're getting the latest versions.
# Check https://github.com/nodejs/docker-node/tree/b4117f9333da4138b03a546ec926ef50a31506c3#nodealpine to understand why libc6-compat might be needed.
RUN apk update && apk upgrade --no-cache libcrypto3 libssl3 libc6-compat busybox ssl_client zlib

# ==================================================================================================

FROM --platform=linux/amd64 alpine AS build-base
# Pin turbo to avoid nondeterministic prune output from future patch releases.
RUN npm install turbo@2.9.18 --global
ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable
RUN corepack prepare pnpm@11.4.0 --activate