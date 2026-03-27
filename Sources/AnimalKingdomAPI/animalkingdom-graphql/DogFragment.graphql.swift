// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

struct DogFragment: TestSchema.SelectionSet, Fragment {
  let __data: DataDict
  init(_dataDict: DataDict) { __data = _dataDict }

  static var __parentType: any Apollo.ParentType { TestSchema.Objects.Dog }
  static var __selections: [Apollo.Selection] { [
    .field("__typename", String.self),
    .field("species", String.self),
  ] }

  var species: String { __data["species"] }

  init(
    species: String
  ) {
    self.init(_dataDict: DataDict(
      data: [
        "__typename": TestSchema.Objects.Dog.typename,
        "species": species,
      ],
      fulfilledFragments: [
        ObjectIdentifier(DogFragment.self)
      ]
    ))
  }
}
