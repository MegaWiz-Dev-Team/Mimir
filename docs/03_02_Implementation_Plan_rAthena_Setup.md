# 🏰 Implementation Plan: rAthena Server Emulator (Mac M3)

This plan details the installation of the rAthena Server Emulator to provide the game data layer for Project-Mimir.

## Assessment: MacBook Air M3 (8/16GB+ RAM)
- **Feasibility**: **Highly Feasible**. rAthena is a lightweight C++ application.
- **Compatibility**: Runs well on macOS (ARM64) via Docker or native compilation. Docker is recommended for environment isolation.
- **Resource Impact**: Estimated < 2GB RAM and < 5% CPU for a development environment.

## Proposed Changes

### [New] rAthena Repository
- Clone [rAthena/rathena](https://github.com/rathena/rathena) into `./rathena`.

### [New] Docker Configuration `rathena.Dockerfile`
- Base image: `ubuntu:22.04` (for stability).
- Package: `build-essential`, `cmake`, `git`, `mariadb-client`, `libmariadb-dev`, `zlib1g-dev`, `libpcre3-dev`.
- Multi-stage build to compile and then run the binaries.

### [Modify] [docker-compose.yml](file:///Volumes/T7%20Shield/Development/Active_Projects/project/Project-Mimir/docker-compose.yml)
- Add services for `login-server`, `char-server`, and `map-server`.
- Map rAthena SQL scripts to the database initialization.

### [Database] MariaDB Setup
- Import rAthena schemas into the existing `ro_landverse` database (or a new `rathena` database).
- **Critical**: Use the `data/mariadb` volume for persistence.

## Execution Steps

1. **Clone**: `git clone https://github.com/rathena/rathena.git`
2. **Database Init**: Run rAthena's `.sql` scripts against `mimir_mariadb`.
3. **Configuration**: 
    - Adjust `conf/inter_athena.conf` (DB connection).
    - Adjust `conf/char_athena.conf` and `conf/map_athena.conf`.
4. **Compile & Run**: Use `docker-compose up --build`.

## Verification Plan

### Manual Verification
- **Process Check**: Ensure all 3 servers (login, char, map) stay running and connected to MariaDB.
- **Data Check**: Verify that `item_db` and `mob_db` tables are populated.
- **API Check**: Verify that the `ro-ai-bridge` can query the rAthena tables.
