import { lazy, Suspense } from 'react'
import EngineerApp from './engineer/EngineerApp'
const PitwallApp=lazy(()=>import('./PitwallApp'))
export default function App(){return location.pathname==='/legacy'?<Suspense fallback={<p>Loading Pitwall…</p>}><PitwallApp/></Suspense>:<EngineerApp/>}
