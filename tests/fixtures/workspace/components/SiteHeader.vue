<script setup lang="ts">
const links = [
  { to: '/', label: 'Home' },
  { to: '/about', label: 'About' },
]
</script>

<template>
  <header>
    <NuxtLink v-for="l in links" :key="l.to" :to="l.to">{{ l.label }}</NuxtLink>
  </header>
</template>
