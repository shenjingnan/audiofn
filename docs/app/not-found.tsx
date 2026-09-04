export default function NotFound() {
  return (
    <div className="flex min-h-[50vh] flex-col items-center justify-center gap-2 text-center">
      <p className="text-4xl font-bold">404</p>
      <p className="text-sm text-fd-muted-foreground">页面不存在或已被移动</p>
    </div>
  );
}
