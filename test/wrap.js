let test = require('ava')
let wrap = require('../src/wrap.js')
let { deepEqual } = require('./common.js')

test('set non-object on root', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => { mutations = [] }

  wrapper.foo = 'bar'
  deepEqual(t, mutations, [
    {
      op: 'put',
      path: [],
      oldValue: {},
      newValue: { foo: 'bar' },
      existed: true
    }
  ])
  deepEqual(t, obj, { foo: 'bar' })
  deepEqual(t, wrapper, { foo: 'bar' })
})

test('set object on root', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => { mutations = [] }

  wrapper.foo = { x: 5 }
  deepEqual(t, mutations, [
    {
      op: 'put',
      path: [ 'foo' ],
      oldValue: undefined,
      newValue: { x: 5 },
      existed: false
    }
  ])
  deepEqual(t, obj, { foo: { x: 5 } })
  deepEqual(t, wrapper, { foo: { x: 5 } })
})

test('mutate root non-object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => { mutations = [] }

  wrapper.foo = 'bar'

  reset()
  wrapper.foo = 'bar2'
  deepEqual(t, mutations, [
    {
      op: 'put',
      path: [],
      oldValue: { foo: 'bar' },
      newValue: { foo: 'bar2' },
      existed: true
    }
  ])
  deepEqual(t, obj, { foo: 'bar2' })
  deepEqual(t, wrapper, { foo: 'bar2' })
})

test('replace root non-object with object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => { mutations = [] }

  wrapper.foo = 'bar'

  reset()
  wrapper.foo = { x: 5 }
  deepEqual(t, mutations, [
    {
      op: 'put',
      path: [],
      oldValue: { foo: 'bar' },
      newValue: {},
      existed: true
    }, {
      op: 'put',
      path: [ 'foo' ],
      oldValue: 'bar',
      newValue: { x: 5 },
      existed: true
    }
  ])
  deepEqual(t, obj, { foo: { x: 5 } })
  deepEqual(t, wrapper, { foo: { x: 5 } })
})

test('replace object with non-object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => { mutations = [] }

  wrapper.foo = { x: 5 }

  reset()
  wrapper.foo = 'bar'
  deepEqual(t, mutations, [
    {
      op: 'del',
      path: [ 'foo' ],
      oldValue: { x: 5 },
      newValue: undefined,
      existed: true
    }, {
      op: 'put',
      path: [],
      oldValue: {},
      newValue: { foo: 'bar' },
      existed: true
    }
  ])
  deepEqual(t, obj, { foo: 'bar' })
  deepEqual(t, wrapper, { foo: 'bar' })
})

test('mutate non-root object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => { mutations = [] }

  wrapper.foo = { x: 5 }

  reset()
  wrapper.foo.x++
  deepEqual(t, mutations, [
    {
      op: 'put',
      path: [ 'foo' ],
      oldValue: { x: 5 },
      newValue: { x: 6 },
      existed: true
    }
  ])
  deepEqual(t, obj, { foo: { x: 6 } })
  deepEqual(t, wrapper, { foo: { x: 6 } })
})

test('delete object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => { mutations = [] }

  wrapper.foo = { x: 5 }

  reset()
  delete wrapper.foo
  deepEqual(t, mutations, [
    {
      op: 'del',
      path: [ 'foo' ],
      oldValue: { x: 5 },
      newValue: undefined,
      existed: true
    }
  ])
  deepEqual(t, obj, {})
  deepEqual(t, wrapper, {})
})

test('delete root non-object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => { mutations = [] }

  wrapper.foo = 'bar'

  reset()
  delete wrapper.foo
  deepEqual(t, mutations, [
    {
      op: 'put',
      path: [],
      oldValue: { foo: 'bar' },
      newValue: {},
      existed: true
    }
  ])
  deepEqual(t, obj, {})
  deepEqual(t, wrapper, {})
})

test('delete non-root non-object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => { mutations = [] }

  wrapper.foo = { x: 5 }

  reset()
  delete wrapper.foo.x
  deepEqual(t, mutations, [
    {
      op: 'put',
      path: [ 'foo' ],
      oldValue: { x: 5 },
      newValue: {},
      existed: true
    }
  ])
  deepEqual(t, obj, { foo: {} })
  deepEqual(t, wrapper, { foo: {} })
})

test('override object by setting on parent', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => { mutations = [] }

  wrapper.foo = { x: { y: 5 } }

  reset()
  wrapper.foo = { x: 5 }
  deepEqual(t, mutations, [
    {
      op: 'put',
      path: [ 'foo' ],
      oldValue: {},
      newValue: { x: 5 },
      existed: true
    }, {
      op: 'del',
      path: [ 'foo', 'x' ],
      oldValue: { y: 5 },
      newValue: undefined,
      existed: true
    }
  ])
  deepEqual(t, obj, { foo: { x: 5 } })
  deepEqual(t, wrapper, { foo: { x: 5 } })
})

test('set multiple-level object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => { mutations = [] }

  wrapper.foo = { x: { y: 5 } }
  deepEqual(t, mutations, [
    {
      op: 'put',
      path: [ 'foo' ],
      oldValue: undefined,
      newValue: {},
      existed: false
    }, {
      op: 'put',
      path: [ 'foo', 'x' ],
      oldValue: undefined,
      newValue: { y: 5 },
      existed: false
    }
  ])
  deepEqual(t, obj, { foo: { x: { y: 5 } } })
  deepEqual(t, wrapper, { foo: { x: { y: 5 } } })
})

test('delete multiple-level object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => { mutations = [] }

  wrapper.foo = { x: { y: 5 } }

  reset()
  delete wrapper.foo
  deepEqual(t, mutations, [
    {
      op: 'del',
      path: [ 'foo', 'x' ],
      oldValue: { y: 5 },
      newValue: undefined,
      existed: true
    }, {
      op: 'del',
      path: [ 'foo' ],
      oldValue: {},
      newValue: undefined,
      existed: true
    }
  ])
  deepEqual(t, obj, {})
  deepEqual(t, wrapper, {})
})

test('delete non-existent key', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))

  delete wrapper.foo

  deepEqual(t, mutations, [])
  deepEqual(t, obj, {})
  deepEqual(t, wrapper, {})
})

test('functions are bound to parent', (t) => {
  let mutations = []
  let obj = { array: [] }
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))

  wrapper.array.push(123)

  // XXX wonky mutation log
  deepEqual(t, mutations, [
    {
      existed: true,
      newValue: { 0: 123, length: 0 },
      oldValue: { length: 0 },
      op: 'put',
      path: [ 'array' ]
    }, {
      existed: true,
      newValue: { 0: 123, length: 1 },
      oldValue: { 0: 123, length: 1 },
      op: 'put',
      path: [ 'array' ]
    }
  ])
  deepEqual(t, obj, { array: [ 123 ] })
  deepEqual(t, wrapper, { array: [ 123 ] })
})
