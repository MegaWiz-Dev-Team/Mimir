import { NextRequest, NextResponse } from 'next/server';

/**
 * Proxy endpoint for Bifrost agent requests.
 * Allows browser to access Bifrost through the Dashboard without DNS resolution issues.
 *
 * This solves the problem where:
 * - Browser (outside K8s) can't resolve bifrost.asgard.svc (K8s internal DNS)
 * - bifrost.asgard.internal may not be in user's DNS/hosts
 *
 * By proxying through the Dashboard (which runs in K8s), we can use the
 * internal K8s service name bifrost.asgard.svc
 */
export async function GET(request: NextRequest) {
  try {
    // Get tenant from query param or header
    const tenantId = request.headers.get('X-Tenant-Id') || 'asgard_medical';

    // Call Bifrost internally (uses K8s service DNS)
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
        { status: response.status }
      );
    }

    const data = await response.json();
    return NextResponse.json(data);
  } catch (error) {
    console.error('[API] Bifrost proxy error:', error);
    return NextResponse.json(
      { error: 'Failed to fetch agents from Bifrost' },
      { status: 500 }
    );
  }
}
