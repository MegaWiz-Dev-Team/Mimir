# Asgard AI Platform 

## System Network & DNS Architecture (ISO/IEC 27001:2022 Compliant)
**Classification**: CONFIDENTIAL  
**Version**: 1.0.0  

---

### 1. Executive Summary
This document outlines the zero-trust network and Domain Name System (DNS) architecture for the Asgard AI Platform in enterprise B2B deployments. The architecture ensures strict regulatory compliance by keeping internal system communications completely isolated from public DNS resolvers, mitigating the risk of DNS poisoning and traffic interception.

### 2. Conceptual Network Architecture

The architecture utilizes a Site-to-Site Virtual Private Network (VPN) with conditional DNS forwarding. This ensures that client workstations require zero agent installations, maintaining clean endpoints.

```mermaid
graph TD
    %% Define Styles
    classDef client fill:#e1f5fe,stroke:#0288d1,stroke-width:2px,color:#000
    classDef vpn fill:#cfd8dc,stroke:#455a64,stroke-width:2px,stroke-dasharray: 5 5,color:#000
    classDef asgard fill:#e8f5e9,stroke:#388e3c,stroke-width:2px,color:#000
    classDef service fill:#fff,stroke:#4caf50,stroke-width:1px,color:#000

    subgraph Office["Enterprise Client Headquarter"]
        User["💻 Employee Workstations \n (No Client Agent Required)"]:::client
        Router["🛡️ Enterprise Firewall / Gateway \n (DNS Conditional Forwarder)"]:::client
    end

    subgraph Tunnel["Encrypted Tunnel"]
        WG(("🔒 Site-to-Site \n VPN Tunnel \n (e.g., WireGuard/IPSec)")):::vpn
    end

    subgraph Datacenter["Enterprise Datacenter (Asgard Hosted Environment)"]
        CoreDNS["⚙️ Asgard CoreDNS \n (Authoritative for .asgard.internal)"]:::asgard
        Ingress["🚦 Traefik Ingress Controller \n (TLS Termination & Routing)"]:::asgard
        
        subgraph Microservices["Isolated Microservices"]
            UI["mimir-dashboard"]:::service
            SSO["zitadel-sso"]:::service
            API["mimir-core-api"]:::service
        end
    end

    %% Network Flow
    User -->|1. Web Request \n (mimir.asgard.internal)| Router
    Router -->|2. Forward DNS Query| WG
    WG -.->|3. Resolve UDP/53| CoreDNS
    CoreDNS -.->|4. Returns K3s IP| Router
    
    User ==>|5. Local Network Route| WG
    WG ==>|6. Encrypted Payload| Ingress
    
    Ingress == "Host: mimir..." ==> UI
    Ingress == "Host: sso..." ==> SSO
    Ingress == "Host: api..." ==> API
```

### 3. DNS Implementation & Convention 

To establish absolute network segmentation, the deployment strictly leverages the `.asgard.internal` domain namespace. CoreDNS within the local cluster holds the authoritative zone map. 

#### Service Subdomain Mapping
| Service Component | Subdomain | Internal Port | Protocol | Purpose |
| :--- | :--- | :--- | :--- | :--- |
| **User Dashboard** | `mimir.asgard.internal` | 80/443 | HTTP(S) | Web Interface for end-users |
| **Authentication** | `sso.asgard.internal` | 80/443 | HTTP(S) | OIDC/OAuth2 Authentication |
| **Core API Gateway**| `api.asgard.internal` | 80/443 | HTTP(S) | LLM and Integration Endpoints |

#### CoreDNS Zone Configuration
The DNS resolver is hardened by patching the orchestrator’s `ConfigMap` to statically bind subdomains to the internal load-balancer ingress IP (e.g., `10.100.x.x`). Traffic never leaks to root servers.

```text
# Secure Zone Declaration
asgard.internal:53 {
    errors
    cache 30
    rewrite name exact mimir.asgard.internal 10.100.0.10
    rewrite name exact sso.asgard.internal   10.100.0.10
    rewrite name exact api.asgard.internal   10.100.0.10
    forward . /etc/resolv.conf
}
```

### 4. Security & Compliance (ISO 27001 Context)
- **A.13.1.1 (Network Controls)**: All DNS and HTTP traffic is encapsulated within an encrypted tunnel. Private domain naming averts external visibility mapping (Reconnaissance Defense).
- **A.9.4.2 (Secure Log-on Procedures)**: The adoption of `sso.asgard.internal` enables strict Origin controls for OIDC/PKCE logic on internal network, ensuring uncompromised Single Sign-On operations.
- **A.14.1.2 (Securing Application Services on Public Networks)**: Since the endpoints reside entirely within `.internal` domains, Asgard is cryptographically invisible from the public internet.

---
*End of Document*
