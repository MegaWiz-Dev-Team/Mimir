const jwt = require('jsonwebtoken');
const token = jwt.sign({
    sub: "testuser",
    tenant_id: "default_tenant",
    role: "admin",
    exp: Math.floor(Date.now() / 1000) + (60 * 60)
}, 'dev_secret_key');

async function runTests() {
    const headers = {
        'Authorization': `Bearer ${token}`,
        'Content-Type': 'application/json'
    };

    console.log("=== TC_SP2_01 & TC_SP2_02: Tenant Settings ===");
    // Get Settings
    let res = await fetch('http://localhost:8080/api/v1/tenant/settings', { headers });
    let data = await res.json();
    console.log("GET /settings:", res.status, data);
    
    // Update Settings
    res = await fetch('http://localhost:8080/api/v1/tenant/settings', { 
        method: 'PUT',
        headers,
        body: JSON.stringify({ name: "Mimir Default Tenant (Updated)" })
    });
    console.log("PUT /settings:", res.status, await res.text());

    console.log("\n=== TC_SP2_03: Data Isolation - API Filtering ===");
    res = await fetch('http://localhost:8080/api/v1/vector/search', {
        method: 'POST',
        headers,
        body: JSON.stringify({ query: "test", limit: 2 })
    });
    let searchRes = await res.json();
    console.log("POST /vector/search:", res.status, "found:", searchRes.length || 0, "results");

    console.log("\n=== TC_SP2_04: Vector Management UI Updates (Delete functionality) ===");
    res = await fetch('http://localhost:8080/api/v1/vector/999999', {
        method: 'DELETE',
        headers
    });
    console.log("DELETE /vector/999999:", res.status, await res.text());
}
runTests();
