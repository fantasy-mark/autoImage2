FROM --platform=${BUILDPLATFORM} golang:1.26 AS migrate-builder
ARG TARGETOS
ARG TARGETARCH
ENV CGO_ENABLED=0 \
    GOBIN=/out \
    GOOS=${TARGETOS} \
    GOARCH=${TARGETARCH}
# Build only the ClickHouse migrate CLI Langfuse uses at runtime.
# compile this ourselves instead of downloading the upstream release
# because prebuilt bins bundle many unused drivers and thus inherit CVEs
# eg.: https://github.com/golang-migrate/migrate/issues/1357
RUN /usr/local/go/bin/go install -trimpath -tags 'clickhouse' -ldflags='-s -w' \
    github.com/golang-migrate/migrate/v4/cmd/migrate@v4.19.1