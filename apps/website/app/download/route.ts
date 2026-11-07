import { NextResponse } from "next/server";
import { getLatestRelease } from "@/lib/releases";
import { site } from "@/lib/site";

// The redirect follows the latest release and is cached with it for an hour.
export const revalidate = 3600;

export async function GET() {
  const release = await getLatestRelease();
  return NextResponse.redirect(release?.dmgUrl ?? site.releases, 307);
}
