import { useEffect, useRef, useState } from "react";
import {
  Alert,
  KeyboardAvoidingView,
  Modal,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";
import MapView, { Marker } from "react-native-maps";
import AsyncStorage from "@react-native-async-storage/async-storage";
import * as Location from "expo-location";
import * as Speech from "expo-speech";
import { DEMO_CAMERAS, Camera } from "../src/cameras";

const ALERT_M = 402.336;
const STORAGE_KEY = "roadwatch:test-drafts:v1";

type Draft = {
  id: string;
  label: string;
  kind: string;
  notes: string;
  latitude: number;
  longitude: number;
  accuracy: number | null;
  capturedAt: string;
  savedAt: string;
  test: true;
};

function isDraft(value: unknown): value is Draft {
  if (!value || typeof value !== "object") return false;
  const d = value as Draft;
  return (
    typeof d.id === "string" &&
    typeof d.label === "string" &&
    typeof d.kind === "string" &&
    typeof d.notes === "string" &&
    Number.isFinite(d.latitude) &&
    Math.abs(d.latitude) <= 90 &&
    Number.isFinite(d.longitude) &&
    Math.abs(d.longitude) <= 180 &&
    (d.accuracy === null ||
      (Number.isFinite(d.accuracy) && d.accuracy >= 0)) &&
    typeof d.capturedAt === "string" &&
    typeof d.savedAt === "string" &&
    d.test === true
  );
}

function meters(
  a: { latitude: number; longitude: number },
  b: Camera
) {
  const radians = Math.PI / 180;
  const p1 = a.latitude * radians;
  const p2 = b.latitude * radians;
  const dp = (b.latitude - a.latitude) * radians;
  const dl = (b.longitude - a.longitude) * radians;
  const x =
    Math.sin(dp / 2) ** 2 +
    Math.cos(p1) * Math.cos(p2) * Math.sin(dl / 2) ** 2;
  const bounded = Math.max(0, Math.min(1, x));
  return (
    2 *
    6371008.8 *
    Math.atan2(Math.sqrt(bounded), Math.sqrt(1 - bounded))
  );
}

export default function Home() {
  const [fix, setFix] = useState<Location.LocationObject | null>(null);
  const [status, setStatus] = useState("Requesting location...");
  const [last, setLast] = useState<string | null>(null);
  const [drafts, setDrafts] = useState<Draft[]>([]);
  const [storageReady, setStorageReady] = useState(false);
  const [storageError, setStorageError] = useState(false);
  const [screen, setScreen] = useState<"report" | "drafts" | null>(null);
  const [capture, setCapture] =
    useState<Location.LocationObject | null>(null);
  const [label, setLabel] = useState("");
  const [kind, setKind] = useState("Unknown");
  const [notes, setNotes] = useState("");
  const [saving, setSaving] = useState(false);
  const saveLock = useRef(false);
  const spoken = useRef(new Set<string>());
  const pos = fix?.coords;

  useEffect(() => {
    let active = true;

    async function loadDrafts() {
      try {
        const raw = await AsyncStorage.getItem(STORAGE_KEY);
        const parsed: unknown = raw === null ? [] : JSON.parse(raw);
        if (!Array.isArray(parsed) || !parsed.every(isDraft)) {
          throw new Error("Invalid saved drafts");
        }
        if (active) {
          setDrafts(parsed);
          setStorageReady(true);
        }
      } catch {
        if (active) setStorageError(true);
      }
    }

    void loadDrafts();
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    let subscription: Location.LocationSubscription | undefined;

    async function startLocation() {
      try {
        const permission =
          await Location.requestForegroundPermissionsAsync();
        if (!active) return;
        if (permission.status !== "granted") {
          setStatus("Location permission is required.");
          return;
        }

        const first = await Location.getCurrentPositionAsync({
          accuracy: Location.Accuracy.High,
        });
        if (!active) return;
        setFix(first);
        setStatus("Demo monitoring active");

        const watcher = await Location.watchPositionAsync(
          {
            accuracy: Location.Accuracy.High,
            distanceInterval: 10,
          },
          (next) => {
            if (active) setFix(next);
          }
        );

        if (active) subscription = watcher;
        else watcher.remove();
      } catch {
        if (active) {
          setStatus("Location unavailable. Check permissions and reload.");
        }
      }
    }

    void startLocation();
    return () => {
      active = false;
      subscription?.remove();
      void Speech.stop();
    };
  }, []);

  useEffect(() => {
    if (!pos) return;
    const near = DEMO_CAMERAS
      .map((camera) => ({ camera, distance: meters(pos, camera) }))
      .filter((item) => item.distance <= ALERT_M)
      .sort((a, b) => a.distance - b.distance)[0];

    if (near && !spoken.current.has(near.camera.id)) {
      spoken.current.add(near.camera.id);
      const message =
        `Demo camera nearby: ${near.camera.label}, approximately ` +
        `${Math.max(10, Math.round(near.distance / 10) * 10)} meters.`;
      setLast(message);
     
setLast(message);
Speech.speak(String(message));
    }
  }, [pos]);

  function openReport() {
    if (!storageReady) {
      Alert.alert(
        "Draft storage unavailable",
        storageError
          ? "Saved drafts could not be read. Reload the app before trying again."
          : "Please wait for saved drafts to finish loading."
      );
      return;
    }

    Alert.alert(
      "Report a test camera",
      "Open the form only when safely stopped.",
      [
        { text: "Cancel", style: "cancel" },
        {
          text: "I am stopped",
          onPress: () => {
            setCapture(null);
            setLabel("");
            setKind("Unknown");
            setNotes("");
            setScreen("report");
          },
        },
      ]
    );
  }

  function captureLocation() {
    if (!fix || Date.now() - fix.timestamp > 60000) {
      Alert.alert(
        "Fresh location needed",
        "Your location is missing or over one minute old. Return to the map and wait for a fresh location, or reload."
      );
      return;
    }
    setCapture(fix);
  }

  async function saveDraft() {
    if (saveLock.current) return;

    if (!label.trim()) {
      Alert.alert("Add a label", "For example: TEST camera.");
      return;
    }
    if (!capture) {
      Alert.alert("Location needed", "Tap Use my current location first.");
      return;
    }
    if (!storageReady) {
      Alert.alert("Storage unavailable", "Reload before saving.");
      return;
    }

    saveLock.current = true;
    setSaving(true);

    const draft: Draft = {
      id: `test-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`,
      label: label.trim(),
      kind: kind.trim() || "Unknown",
      notes: notes.trim(),
      latitude: capture.coords.latitude,
      longitude: capture.coords.longitude,
      accuracy: capture.coords.accuracy,
      capturedAt: new Date(capture.timestamp).toISOString(),
      savedAt: new Date().toISOString(),
      test: true,
    };

    try {
      const updated = [...drafts, draft];
      await AsyncStorage.setItem(STORAGE_KEY, JSON.stringify(updated));
      setDrafts(updated);
      setScreen(null);
      Alert.alert(
        "Test draft saved",
        "Saved on this phone. It has not been submitted or verified."
      );
    } catch {
      Alert.alert(
        "Not saved",
        "Storage failed. Your form is still open; please try again."
      );
    } finally {
      saveLock.current = false;
      setSaving(false);
    }
  }

  function closeModal() {
    if (!saveLock.current) setScreen(null);
  }

  const region = {
    latitude: pos?.latitude ?? 33.521,
    longitude: pos?.longitude ?? -84.354,
    latitudeDelta: 0.03,
    longitudeDelta: 0.03,
  };

  return (
    <View style={styles.root}>
      <MapView style={styles.map} region={region} showsUserLocation>
        {DEMO_CAMERAS.map((camera) => (
          <Marker
            key={camera.id}
            coordinate={camera}
            title={camera.label}
            description="Demo fixture — not a real camera location"
          />
        ))}
        {drafts.map((draft) => (
          <Marker
            key={draft.id}
            coordinate={draft}
            pinColor="orange"
            title={`TEST: ${draft.label}`}
            description={`${draft.kind} — local draft, unverified`}
          />
        ))}
      </MapView>

      <View style={styles.panel}>
        <Text style={styles.title}>ROADWATCH · DEMO v0.2</Text>
        <Text>{status}</Text>
        {last && <Text style={styles.alert}>{last}</Text>}
        {storageError && (
          <Text style={styles.error}>
            Could not load drafts. Saving is disabled to protect existing data.
          </Text>
        )}

        <Pressable
          accessibilityRole="button"
          style={styles.button}
          onPress={openReport}
        >
          <Text style={styles.buttonText}>REPORT CAMERA</Text>
        </Pressable>

        <Pressable
          accessibilityRole="button"
          style={styles.secondary}
          onPress={() => setScreen("drafts")}
        >
          <Text style={styles.secondaryText}>
            VIEW TEST DRAFTS ({drafts.length})
          </Text>
        </Pressable>

        <Text style={styles.note}>
          Demo fixtures and local test drafts only. Use while safely stopped.
        </Text>
      </View>

      <Modal
        visible={screen !== null}
        animationType="slide"
        onRequestClose={closeModal}
      >
        <KeyboardAvoidingView
          style={styles.root}
          behavior={Platform.OS === "ios" ? "padding" : undefined}
        >
          <ScrollView
            contentContainerStyle={styles.form}
            keyboardShouldPersistTaps="handled"
          >
            <Text style={styles.title}>
              {screen === "report" ? "New test draft" : "Saved test drafts"}
            </Text>

            <Text>
              Stored on this phone only. Not submitted, verified, or included
              in camera alerts.
            </Text>

            {screen === "report" ? (
              <>
                <Text style={styles.fieldLabel}>Label *</Text>
                <TextInput
                  accessibilityLabel="Report label"
                  style={styles.input}
                  value={label}
                  onChangeText={setLabel}
                  placeholder="TEST camera"
                  maxLength={80}
                  editable={!saving}
                />

                <Text style={styles.fieldLabel}>Camera type</Text>
                <TextInput
                  accessibilityLabel="Camera type"
                  style={styles.input}
                  value={kind}
                  onChangeText={setKind}
                  placeholder="Unknown"
                  maxLength={80}
                  editable={!saving}
                />

                <Text style={styles.fieldLabel}>Notes</Text>
                <TextInput
                  accessibilityLabel="Report notes"
                  style={[styles.input, styles.notes]}
                  value={notes}
                  onChangeText={setNotes}
                  placeholder="Describe this test report"
                  maxLength={500}
                  multiline
                  editable={!saving}
                />

                <Pressable
                  accessibilityRole="button"
                  style={styles.secondary}
                  disabled={saving}
                  onPress={captureLocation}
                >
                  <Text style={styles.secondaryText}>
                    USE MY CURRENT LOCATION
                  </Text>
                </Pressable>

                <Text>
                  This uses your phone's position, not a separately selected
                  camera position.
                </Text>

                {capture && (
                  <Text>
                    Captured: {capture.coords.latitude.toFixed(6)},{" "}
                    {capture.coords.longitude.toFixed(6)}
                    {"\n"}Estimated accuracy:{" "}
                    {capture.coords.accuracy === null
                      ? "unknown"
                      : `${Math.round(capture.coords.accuracy)} meters`}
                  </Text>
                )}

                <Pressable
                  accessibilityRole="button"
                  style={[styles.button, saving && styles.disabled]}
                  disabled={saving}
                  onPress={() => void saveDraft()}
                >
                  <Text style={styles.buttonText}>
                    {saving ? "SAVING..." : "SAVE TEST DRAFT"}
                  </Text>
                </Pressable>
              </>
            ) : (
              <>
                {!storageReady && (
                  <Text>
                    {storageError
                      ? "Drafts could not be loaded."
                      : "Loading drafts..."}
                  </Text>
                )}
                {storageReady && drafts.length === 0 && (
                  <Text>No test drafts saved yet.</Text>
                )}
                {drafts.map((draft) => (
                  <View key={draft.id} style={styles.card}>
                    <Text style={styles.fieldLabel}>TEST: {draft.label}</Text>
                    <Text>Type: {draft.kind}</Text>
                    <Text>
                      {draft.latitude.toFixed(6)},{" "}
                      {draft.longitude.toFixed(6)}
                    </Text>
                    {!!draft.notes && <Text>{draft.notes}</Text>}
                    <Text>
                      Saved: {new Date(draft.savedAt).toLocaleString()}
                    </Text>
                    <Text>Local draft · Unverified</Text>
                  </View>
                ))}
              </>
            )}

            <Pressable
              accessibilityRole="button"
              style={styles.secondary}
              disabled={saving}
              onPress={closeModal}
            >
              <Text style={styles.secondaryText}>
                {screen === "report" ? "CANCEL" : "BACK TO MAP"}
              </Text>
            </Pressable>
          </ScrollView>
        </KeyboardAvoidingView>
      </Modal>
    </View>
  );
}

const styles = StyleSheet.create({
  root: { flex: 1, backgroundColor: "white" },
  map: { flex: 1 },
  panel: {
    padding: 18,
    paddingBottom: 30,
    gap: 8,
    backgroundColor: "white",
  },
  title: { fontSize: 20, fontWeight: "800", color: "#111" },
  alert: { fontWeight: "700" },
  error: { color: "#a32121" },
  button: {
    padding: 14,
    borderRadius: 10,
    backgroundColor: "#111",
  },
  buttonText: {
    color: "white",
    textAlign: "center",
    fontWeight: "800",
  },
  secondary: {
    padding: 14,
    borderRadius: 10,
    backgroundColor: "#e8edf3",
  },
  secondaryText: {
    color: "#111",
    textAlign: "center",
    fontWeight: "700",
  },
  note: { fontSize: 12 },
  form: {
    padding: 22,
    paddingTop: 60,
    paddingBottom: 50,
    gap: 14,
  },
  fieldLabel: { fontWeight: "700", color: "#111" },
  input: {
    borderWidth: 1,
    borderColor: "#888",
    borderRadius: 8,
    padding: 12,
    fontSize: 16,
    color: "#111",
    backgroundColor: "white",
  },
  notes: { minHeight: 90, textAlignVertical: "top" },
  card: {
    padding: 14,
    borderRadius: 10,
    backgroundColor: "#f1f4f7",
    gap: 6,
  },
  disabled: { opacity: 0.5 },
});