let test = require('ava')
let wrap = require('../src/wrap.js')

test('set non-object on root', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => mutations = []

  wrapper.foo = 'bar'
  t.deepEqual(mutations, [
    {
      op: 'put',
      path: [],
      oldValue: {},
      newValue: { foo: 'bar' },
      existed: true
    }
  ])
  t.deepEqual(obj, { foo: 'bar' })
  t.deepEqual(wrapper, { foo: 'bar' })
})

test('set object on root', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => mutations = []

  wrapper.foo = { x: 5 }
  t.deepEqual(mutations, [
    {
      op: 'put',
      path: [ 'foo' ],
      oldValue: undefined,
      newValue: { x: 5 },
      existed: false
    }
  ])
  t.deepEqual(obj, { foo: { x: 5 } })
  t.deepEqual(wrapper, { foo: { x: 5 } })
})

test('mutate root non-object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => mutations = []

  wrapper.foo = 'bar'

  reset()
  wrapper.foo = 'bar2'
  t.deepEqual(mutations, [
    {
      op: 'put',
      path: [],
      oldValue: { foo: 'bar' },
      newValue: { foo: 'bar2' },
      existed: true
    }
  ])
  t.deepEqual(obj, { foo: 'bar2' })
  t.deepEqual(wrapper, { foo: 'bar2' })
})

test('replace root non-object with object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => mutations = []

  wrapper.foo = 'bar'

  reset()
  wrapper.foo = { x: 5 }
  t.deepEqual(mutations, [
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
  t.deepEqual(obj, { foo: { x: 5 } })
  t.deepEqual(wrapper, { foo: { x: 5 } })
})

test('replace object with non-object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => mutations = []

  wrapper.foo = { x: 5 }

  reset()
  wrapper.foo = 'bar'
  t.deepEqual(mutations, [
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
  t.deepEqual(obj, { foo: 'bar' })
  t.deepEqual(wrapper, { foo: 'bar' })
})

test('mutate non-root object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => mutations = []

  wrapper.foo = { x: 5 }

  reset()
  wrapper.foo.x++
  t.deepEqual(mutations, [
    {
      op: 'put',
      path: [ 'foo' ],
      oldValue: { x: 5 },
      newValue: { x: 6 },
      existed: true
    }
  ])
  t.deepEqual(obj, { foo: { x: 6 } })
  t.deepEqual(wrapper, { foo: { x: 6 } })
})

test('delete object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => mutations = []

  wrapper.foo = { x: 5 }

  reset()
  delete wrapper.foo
  t.deepEqual(mutations, [
    {
      op: 'del',
      path: [ 'foo' ],
      oldValue: { x: 5 },
      newValue: undefined,
      existed: true
    }
  ])
  t.deepEqual(obj, {})
  t.deepEqual(wrapper, {})
})

test('delete root non-object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => mutations = []

  wrapper.foo = 'bar'

  reset()
  delete wrapper.foo
  t.deepEqual(mutations, [
    {
      op: 'put',
      path: [],
      oldValue: { foo: 'bar' },
      newValue: {},
      existed: true
    }
  ])
  t.deepEqual(obj, {})
  t.deepEqual(wrapper, {})
})

test('delete non-root non-object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => mutations = []

  wrapper.foo = { x: 5 }

  reset()
  delete wrapper.foo.x
  t.deepEqual(mutations, [
    {
      op: 'put',
      path: [ 'foo' ],
      oldValue: { x: 5 },
      newValue: {},
      existed: true
    }
  ])
  t.deepEqual(obj, { foo: {} })
  t.deepEqual(wrapper, { foo: {} })
})

test('override object by setting on parent', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => mutations = []

  wrapper.foo = { x: { y: 5 } }

  reset()
  wrapper.foo = { x: 5 }
  t.deepEqual(mutations, [
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
  t.deepEqual(obj, { foo: { x: 5 } })
  t.deepEqual(wrapper, { foo: { x: 5 } })
})

test('set multiple-level object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => mutations = []

  wrapper.foo = { x: { y: 5 } }
  t.deepEqual(mutations, [
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
  t.deepEqual(obj, { foo: { x: { y: 5 } } })
  t.deepEqual(wrapper, { foo: { x: { y: 5 } } })
})

test('delete multiple-level object', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))
  let reset = () => mutations = []

  wrapper.foo = { x: { y: 5 } }

  reset()
  delete wrapper.foo
  t.deepEqual(mutations, [
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
  t.deepEqual(obj, {})
  t.deepEqual(wrapper, {})
})

test('delete non-existent key', (t) => {
  let mutations = []
  let obj = {}
  let wrapper = wrap(obj, (mutation) => mutations.push(mutation))

  delete wrapper.foo

  t.deepEqual(mutations, [])
  t.deepEqual(obj, {})
  t.deepEqual(wrapper, {})
})
