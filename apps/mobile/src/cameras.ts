export type CameraKind="alpr"|"speed"|"red_light"|"traffic"|"unknown";
export type Camera={id:string;latitude:number;longitude:number;kind:CameraKind;label:string;verified:boolean};
// Demo fixtures only. Never present these as real-world camera locations.
export const DEMO_CAMERAS:Camera[]=[
{id:"demo-1",latitude:33.5210,longitude:-84.3540,kind:"alpr",label:"Demo ALPR",verified:true},
{id:"demo-2",latitude:33.5240,longitude:-84.3500,kind:"speed",label:"Demo speed camera",verified:false},
{id:"demo-3",latitude:33.5180,longitude:-84.3590,kind:"traffic",label:"Demo traffic camera",verified:true}
];
