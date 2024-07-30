# rust-app

rust-app description

[Crate API Documentation](https://majksa-dev.github.io/rust-app/)

**Table of Contents**

- [Running](#running)
- [App Configuration](#app-configuration)

## Running

<!-- x-release-please-start-version -->

`docker run --rm -p 80:80 ghcr.io/majksa-dev/api-app:0.1.1`

<!-- x-release-please-end -->

## App Configuration

| **ENV**          | **Description**                                              | **Default** |
| ---------------- | ------------------------------------------------------------ | ----------- |
| PORT             | HTTP port that the gateway will be exposed on.               | 80          |
| HEALTHCHECK_PORT | HTTP port that gateway healthcheck endpoint is available on. | 9000        |
