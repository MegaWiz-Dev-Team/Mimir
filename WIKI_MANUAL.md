# Mimir — Asgard User Manual

## System Overview
Core Architecture Platform. Coordinates knowledge integration, Agent Studio execution, and BigQuery vector syncing.

This repository is an integral node of the **Asgard AI Ecosystem**. It operates securely inside the native K3s cluster.

## Architecture
```mermaid
graph TD;
    API[External Request] --> Gateway[Ingress Controller];
    Gateway --> Mimir[Mimir Pod];
    Mimir --> MCP[Hermodr Sidecar];
    MCP --> InternalDB[(Internal Data Source)];
```

## Setup & Deployment
To deploy Mimir natively within the K3s environment, navigate to the Asgard root and execute:
```bash
./scripts/k3s-deploy.sh mimir
```
*Note: In SIT/Local iterations, this service resolves internally at `mimir.asgard.internal` via local `/etc/hosts` DNS configuration.*

## MCP Integration Strategy (Read-only Boundary)
In alignment with platform security parameters, the MCP toolsets exposed by Mimir through the Hermodr sidecar are explicitly restricted to **GET**, **LIST**, and **CHECK** capabilities. 

All transaction-mutating tools (POST/PUT/DELETE) remain structurally disabled at the MCP edge tier to ensure agent immutability during preliminary cluster staging.

## Interface & Usage Flow
*Visual guides and interface demonstrations (where applicable) are appended beneath this line.*

![Mimir Dashboard Capture](/Users/mimir/.gemini/antigravity/brain/3950c60b-fb68-47e6-91ad-a486dc738146/mimir_dashboard_demo_1776573483196.webp)
