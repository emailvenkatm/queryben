import { Navigate, Route, Routes } from 'react-router-dom';

function Home() {
  return <div>home</div>;
}

function Queries() {
  return <div>queries</div>;
}

function History() {
  return <div>history</div>;
}

function Notebooks() {
  return <div>notebooks</div>;
}

function SchemaCompare() {
  return <div>schema compare</div>;
}

function Settings() {
  return <div>settings</div>;
}

export function App() {
  return (
    <Routes>
      <Route path="/" element={<Home />} />
      <Route path="/queries" element={<Queries />} />
      <Route path="/history" element={<History />} />
      <Route path="/notebooks" element={<Notebooks />} />
      <Route path="/schema-compare" element={<SchemaCompare />} />
      <Route path="/settings" element={<Settings />} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  );
}
