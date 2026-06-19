import { NextRequest, NextResponse } from 'next/server';

/**
 * Proxy GET /api/bifrost/agents -> Bifrost (in-cluster).
 *
 * Header-based tenant scoping: this forwards ONLY the X-Tenant-Id header and
 * NO Authorization/JWT, so the SELECTED tenant is honored (Bifrost's
 * require_tenant header-fallback) instead of a JWT-pinned tenant. Runs on the
 * dashboard server so the browser can reach the internal bifrost.asgard.svc.
 *
 * Fail closed: a missing tenant returns 400 (never default to a real tenant).
 */
export async function GET(request: NextRequest) {
  try {
    const tenantId = request.headers.get('X-Tenant-Id');
    if (!tenantId) {
      return NextResponse.json(
        { error: 'missing_tenant', detail: 'X-Tenant-Id header is required' },
        { status: 400 },
      );
    }

    const bifrostUrl = 'http://bifrost.asgard.svc:8100/v1/agents';
    const response = await fetch(bifrostUrl, {
      method: 'GET',
      headers: {
        'X-Tenant-Id': tenantId,
        'Content-Type': 'application/json',
      },
    });

    if (!response.ok) {
      return NextResponse.json(
        { error: `Bifrost error: ${response.statusText}` },
        { status: response.status },
      );
    }

    return NextResponse.json(await response.json());
  } catch (error) {
    console.error('[API] Bifrost agents proxy error:', error);
    return NextResponse.json(
      { error: 'Failed to fetch agents from Bifrost' },
      { status: 500 },
    );
  }
}
