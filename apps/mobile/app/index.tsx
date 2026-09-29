import {useEffect,useRef,useState} from "react";
import {Pressable,StyleSheet,Text,View} from "react-native";
import MapView,{Marker} from "react-native-maps";
import * as Location from "expo-location";
import * as Speech from "expo-speech";
import {DEMO_CAMERAS,Camera} from "../src/cameras";

const ALERT_M=402.336;
function meters(a:{latitude:number;longitude:number},b:Camera){
 const r=6371008.8,p1=a.latitude*Math.PI/180,p2=b.latitude*Math.PI/180;
 const dp=(b.latitude-a.latitude)*Math.PI/180,dl=(b.longitude-a.longitude)*Math.PI/180;
 const x=Math.sin(dp/2)**2+Math.cos(p1)*Math.cos(p2)*Math.sin(dl/2)**2;
 return 2*r*Math.atan2(Math.sqrt(x),Math.sqrt(1-x));
}
export default function Home(){
 const [pos,setPos]=useState<Location.LocationObjectCoords|null>(null);
 const [status,setStatus]=useState("Requesting location…");
 const [last,setLast]=useState<string|null>(null);
 const spoken=useRef(new Set<string>());
 useEffect(()=>{let sub:Location.LocationSubscription|undefined;(async()=>{
  const p=await Location.requestForegroundPermissionsAsync();
  if(p.status!=="granted"){setStatus("Location permission required for proximity alerts.");return;}
  const first=await Location.getCurrentPositionAsync({accuracy:Location.Accuracy.High});setPos(first.coords);setStatus("Demo monitoring active");
  sub=await Location.watchPositionAsync({accuracy:Location.Accuracy.High,distanceInterval:10},l=>setPos(l.coords));
 })();return()=>sub?.remove();},[]);
 useEffect(()=>{if(!pos)return;const near=DEMO_CAMERAS.map(c=>({c,d:meters(pos,c)})).filter(x=>x.d<=ALERT_M).sort((a,b)=>a.d-b.d)[0];
  if(near&&!spoken.current.has(near.c.id)){spoken.current.add(near.c.id);const msg=`${near.c.label} ahead, approximately ${Math.max(100,Math.round(near.d/100)*100)} meters.`;setLast(msg);Speech.speak(msg);}
 },[pos]);
 const region=pos?{latitude:pos.latitude,longitude:pos.longitude,latitudeDelta:.03,longitudeDelta:.03}:{latitude:33.521,longitude:-84.354,latitudeDelta:.03,longitudeDelta:.03};
 return <View style={styles.root}><MapView style={styles.map} region={region} showsUserLocation>
  {DEMO_CAMERAS.map(c=><Marker key={c.id} coordinate={c} title={c.label} description={c.verified?"Verified demo fixture":"Unverified demo fixture"}/>)}
 </MapView><View style={styles.panel}><Text style={styles.title}>ROADWATCH · DEMO v0.1</Text><Text>{status}</Text>{last&&<Text style={styles.alert}>{last}</Text>}
 <Pressable style={styles.button} onPress={()=>setLast("Report captured as demo draft. Add details only while stopped.")}><Text style={styles.buttonText}>REPORT CAMERA</Text></Pressable>
 <Text style={styles.note}>Demo fixtures only — not real camera locations. Do not interact with the app while driving.</Text></View></View>
}
const styles=StyleSheet.create({root:{flex:1},map:{flex:1},panel:{padding:18,gap:8,backgroundColor:"white"},title:{fontSize:20,fontWeight:"800"},alert:{fontWeight:"700"},button:{padding:14,borderRadius:10,backgroundColor:"#111"},buttonText:{color:"white",textAlign:"center",fontWeight:"800"},note:{fontSize:12}});
