export default defineEventHandler((event) => {
  return { message: 'hello', path: event.path }
})
