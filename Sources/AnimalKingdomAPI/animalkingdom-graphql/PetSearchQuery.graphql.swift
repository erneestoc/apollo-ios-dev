// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

class PetSearchQuery: GraphQLQuery {
  static let operationName: String = "PetSearch"
  static let operationDocument: Apollo.OperationDocument = .init(
    operationIdentifier: "ae35ea2cecd3f22c4b587b63920852360b0997d6a35a78f1c8d049dc6def13d2"
  )

  var filters: GraphQLNullable<TestSchema.PetSearchFilters>

  init(filters: GraphQLNullable<TestSchema.PetSearchFilters> = .init(
    TestSchema.PetSearchFilters(
      species: ["Dog", "Cat"],
      size: .init(.small),
      measurements: .init(
        TestSchema.MeasurementsInput(
          height: 10.5,
          weight: 5.0
        )
      )
    )
  )) {
    self.filters = filters
  }

  var __variables: Variables? { ["filters": filters] }

  struct Data: TestSchema.SelectionSet {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    static var __parentType: any Apollo.ParentType { TestSchema.Objects.Query }
    static var __selections: [Apollo.Selection] { [
      .field("pets", [Pet].self, arguments: ["filters": .variable("filters")]),
    ] }

    var pets: [Pet] { __data["pets"] }

    /// Pet
    ///
    /// Parent Type: `Pet`
    struct Pet: TestSchema.SelectionSet {
      let __data: DataDict
      init(_dataDict: DataDict) { __data = _dataDict }

      static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.Pet }
      static var __selections: [Apollo.Selection] { [
        .field("__typename", String.self),
        .field("id", TestSchema.ID.self),
        .field("humanName", String?.self),
      ] }

      var id: TestSchema.ID { __data["id"] }
      var humanName: String? { __data["humanName"] }
    }
  }
}
