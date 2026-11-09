"""Validate decoded Developer ID profile authorization before applying restricted entitlements."""
import datetime
import plistlib
import sys

TEAM = "V3HN8HXZYK"
APP = TEAM + ".sh.saeed.kudzu"
CONTAINER = "iCloud.sh.saeed.kudzu.library"


def validate(profile, now=None):
    if not isinstance(profile, dict) or not isinstance(profile.get("Entitlements"), dict):
        raise ValueError("profile must contain an entitlements dictionary")
    now = now or datetime.datetime.now(datetime.timezone.utc)
    expiration = profile.get("ExpirationDate")
    if not isinstance(expiration, datetime.datetime):
        raise ValueError("profile has no expiration date")
    if expiration.tzinfo is None:
        expiration = expiration.replace(tzinfo=datetime.timezone.utc)
    if expiration <= now:
        raise ValueError("provisioning profile has expired")
    entitlements = profile.get("Entitlements", {})
    teams = profile.get("TeamIdentifier")
    if not isinstance(teams, list) or TEAM not in teams:
        raise ValueError("profile belongs to a different team")
    if entitlements.get("com.apple.application-identifier") != APP:
        raise ValueError("profile application identifier does not match Kudzu")
    if entitlements.get("com.apple.developer.team-identifier") != TEAM:
        raise ValueError("profile team entitlement does not match Kudzu")
    containers = entitlements.get("com.apple.developer.icloud-container-identifiers")
    if not isinstance(containers, list) or CONTAINER not in containers:
        raise ValueError("profile does not authorize Kudzu's exact iCloud container")
    services = entitlements.get("com.apple.developer.icloud-services")
    # Apple-issued Developer ID profiles use a string wildcard here. The app
    # specializes it to CloudKit; exact container/app/team checks remain above.
    if services != "*" and (not isinstance(services, list) or "CloudKit" not in services):
        raise ValueError("profile does not authorize CloudKit")
    environment = entitlements.get("com.apple.developer.icloud-container-environment")
    allowed = [environment] if isinstance(environment, str) else environment
    if not isinstance(allowed, list) or "Production" not in allowed:
        raise ValueError("profile does not authorize Production CloudKit")
    if entitlements.get("com.apple.developer.aps-environment") != "production":
        raise ValueError("profile does not authorize production macOS push notifications")
    certificates = profile.get("DeveloperCertificates")
    if not isinstance(certificates, list) or not certificates or not all(isinstance(cert, bytes) for cert in certificates):
        raise ValueError("profile contains no signing certificates")


if __name__ == "__main__":
    try:
        profile = plistlib.load(sys.stdin.buffer)
        validate(profile)
        print("validated iCloud profile: " + profile.get("Name", "Kudzu"))
    except (ValueError, TypeError, KeyError, plistlib.InvalidFileException) as error:
        sys.exit("error: " + str(error))
