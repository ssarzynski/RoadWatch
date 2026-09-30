# RoadWatch Mobile Demo v0.1

Purpose: prove the end-to-end phone UX before backend deployment.

Current demo:
- requests foreground location permission;
- displays user location and clearly labeled synthetic demo camera fixtures;
- calculates on-device proximity;
- speaks one alert inside ~1/4 mile (402.336 m);
- suppresses repeat alerts per session;
- provides a non-submitting Report Camera UX placeholder.

Safety: demo fixtures are synthetic and must never be described as real camera locations. Do not operate the app while driving.

## Local test gate

From `apps/mobile`:

```bash
npm install
npx expo start
```

Open with the supported Expo development workflow and test while stationary/walking first. Package/API compatibility is not yet locally verified. Backend API integration, bearing/direction filtering, background behavior, and persisted reports come after this visual/proximity demo passes.
