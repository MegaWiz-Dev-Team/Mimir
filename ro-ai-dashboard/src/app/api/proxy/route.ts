import { NextRequest, NextResponse } from 'next/server';

/**
 * Proxy API requests through the Next.js server
 * Browser -> Dashboard (/api/proxy/*) -> Actual API (http://mimir-api.asgard.svc:8080/*)
 * This allows the browser to reach the API without certificate issues
 */
export async function GET(request: NextRequest) {
  try {
    const url = new URL(request.url);
    const pathSegments = url.pathname.split('/api/proxy/').pop() || '';

    // Construct the actual API URL using internal DNS name
    const apiUrl = `http://mimir-api.asgard.svc:8080/${pathSegments}${url.search}`;

    console.log(`[Proxy] GET ${apiUrl}`);

    // Forward headers
    const headers = new Headers();
    if (request.headers.get('x-tenant-id')) {
      headers.set('X-Tenant-Id', request.headers.get('x-tenant-id')!);
    }
    headers.set('Content-Type', 'application/json');

    // Make the request to the actual API
    const response = await fetch(apiUrl, {
      method: 'GET',
      headers,
    });

    // Return the response
    const data = await response.json();
    return NextResponse.json(data, { status: response.status });
  } catch (error) {
    console.error('[Proxy] Error:', error);
    return NextResponse.json(
      { error: 'Failed to proxy request' },
      { status: 500 }
    );
  }
}

export async function POST(request: NextRequest) {
  try {
    const url = new URL(request.url);
    const pathSegments = url.pathname.split('/api/proxy/').pop() || '';

    const apiUrl = `http://mimir-api.asgard.svc:8080/${pathSegments}${url.search}`;
    const body = await request.text();

    console.log(`[Proxy] POST ${apiUrl}`);

    const headers = new Headers();
    if (request.headers.get('x-tenant-id')) {
      headers.set('X-Tenant-Id', request.headers.get('x-tenant-id')!);
    }
    headers.set('Content-Type', 'application/json');

    const response = await fetch(apiUrl, {
      method: 'POST',
      headers,
      body: body || undefined,
    });

    const data = await response.json();
    return NextResponse.json(data, { status: response.status });
  } catch (error) {
    console.error('[Proxy] Error:', error);
    return NextResponse.json(
      { error: 'Failed to proxy request' },
      { status: 500 }
    );
  }
}
