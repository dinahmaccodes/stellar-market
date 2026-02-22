import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

const PROTECTED_ROUTES = ["/dashboard", "/post-job", "/messages", "/profile"];
const AUTH_ROUTES = ["/auth/login", "/auth/register"];

export function middleware(request: NextRequest) {
  const token = request.cookies.get("stellarmarket_jwt")?.value;
  const { pathname } = request.nextUrl;

  // 1. If user is authenticated and tries to access auth pages, redirect to dashboard
  if (token && AUTH_ROUTES.some((route) => pathname.startsWith(route))) {
    return NextResponse.redirect(new URL("/dashboard", request.url));
  }

  // 2. If user is NOT authenticated and tries to access protected pages, redirect to login
  if (!token && PROTECTED_ROUTES.some((route) => pathname.startsWith(route))) {
    return NextResponse.redirect(new URL("/auth/login", request.url));
  }

  return NextResponse.next();
}

export const config = {
  matcher: [
    "/dashboard/:path*",
    "/post-job/:path*",
    "/messages/:path*",
    "/profile/:path*",
    "/auth/:path*",
  ],
};
