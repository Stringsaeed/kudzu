import { NextResponse } from "next/server";
import { getLatestRelease } from "@/lib/releases";
import { site } from "@/lib/site";

export const dynamic = "force-dynamic";

export async function GET() {
  const release = await getLatestRelease();
  return NextResponse.redirect(release?.dmgUrl ?? site.releases, 307);
}
