// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

class AllAnimalsQuery: GraphQLQuery {
  static let operationName: String = "AllAnimalsQuery"
  static let operationDocument: Apollo.OperationDocument = .init(
    operationIdentifier: "4f3f13736114169177c1fc018d5e8c6fed64026dbb5889214b93eee71f08b498"
  )

  init() {}

  struct Data: TestSchema.SelectionSet {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    static var __parentType: any Apollo.ParentType { TestSchema.Objects.Query }
    static var __selections: [Apollo.Selection] { [
      .field("allAnimals", [AllAnimal].self),
    ] }

    var allAnimals: [AllAnimal] { __data["allAnimals"] }

    /// AllAnimal
    ///
    /// Parent Type: `RenamedAnimal`
    struct AllAnimal: TestSchema.SelectionSet {
      let __data: DataDict
      init(_dataDict: DataDict) { __data = _dataDict }

      static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.RenamedAnimal }
      static var __selections: [Apollo.Selection] { [
        .field("__typename", String.self),
        .field("id", TestSchema.ID.self),
        .field("height", Height.self),
        .field("species", String.self),
        .field("skinCovering", GraphQLEnum<TestSchema.SkinCovering>?.self),
        .field("predators", [Predator].self),
        .inlineFragment(AsWarmBlooded.self),
        .inlineFragment(AsPet.self),
        .inlineFragment(AsCat.self),
        .inlineFragment(AsClassroomPet.self),
        .inlineFragment(AsDog.self),
        .fragment(HeightInMeters.self),
      ] }

      var id: TestSchema.ID { __data["id"] }
      var height: Height { __data["height"] }
      var species: String { __data["species"] }
      var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
      var predators: [Predator] { __data["predators"] }

      var asWarmBlooded: AsWarmBlooded? { _asInlineFragment() }
      var asPet: AsPet? { _asInlineFragment() }
      var asCat: AsCat? { _asInlineFragment() }
      var asClassroomPet: AsClassroomPet? { _asInlineFragment() }
      var asDog: AsDog? { _asInlineFragment() }

      struct Fragments: FragmentContainer {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        var heightInMeters: HeightInMeters { _toFragment() }
      }

      /// AllAnimal.Height
      ///
      /// Parent Type: `Height`
      struct Height: TestSchema.SelectionSet {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }
        static var __selections: [Apollo.Selection] { [
          .field("__typename", String.self),
          .field("feet", Int.self),
          .field("inches", Int?.self),
        ] }

        var feet: Int { __data["feet"] }
        var inches: Int? { __data["inches"] }
        var meters: Int { __data["meters"] }
      }

      /// AllAnimal.Predator
      ///
      /// Parent Type: `RenamedAnimal`
      struct Predator: TestSchema.SelectionSet {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.RenamedAnimal }
        static var __selections: [Apollo.Selection] { [
          .field("__typename", String.self),
          .field("species", String.self),
          .inlineFragment(AsWarmBlooded.self),
        ] }

        var species: String { __data["species"] }

        var asWarmBlooded: AsWarmBlooded? { _asInlineFragment() }

        /// AllAnimal.Predator.AsWarmBlooded
        ///
        /// Parent Type: `WarmBlooded`
        struct AsWarmBlooded: TestSchema.InlineFragment {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal.Predator
          static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.WarmBlooded }
          static var __selections: [Apollo.Selection] { [
            .field("predators", [Predator].self),
            .field("laysEggs", Bool.self),
            .fragment(WarmBloodedDetails.self),
          ] }

          var predators: [Predator] { __data["predators"] }
          var laysEggs: Bool { __data["laysEggs"] }
          var species: String { __data["species"] }
          var bodyTemperature: Int { __data["bodyTemperature"] }
          var height: Height { __data["height"] }

          struct Fragments: FragmentContainer {
            let __data: DataDict
            init(_dataDict: DataDict) { __data = _dataDict }

            var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
            var heightInMeters: HeightInMeters { _toFragment() }
          }

          /// AllAnimal.Predator.AsWarmBlooded.Predator
          ///
          /// Parent Type: `RenamedAnimal`
          struct Predator: TestSchema.SelectionSet {
            let __data: DataDict
            init(_dataDict: DataDict) { __data = _dataDict }

            static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.RenamedAnimal }
            static var __selections: [Apollo.Selection] { [
              .field("__typename", String.self),
              .field("species", String.self),
            ] }

            var species: String { __data["species"] }
          }

          typealias Height = HeightInMeters.Height
        }
      }

      /// AllAnimal.AsWarmBlooded
      ///
      /// Parent Type: `WarmBlooded`
      struct AsWarmBlooded: TestSchema.InlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
        static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.WarmBlooded }
        static var __selections: [Apollo.Selection] { [
          .fragment(WarmBloodedDetails.self),
        ] }

        var id: TestSchema.ID { __data["id"] }
        var height: Height { __data["height"] }
        var species: String { __data["species"] }
        var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        var predators: [Predator] { __data["predators"] }
        var bodyTemperature: Int { __data["bodyTemperature"] }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
          var heightInMeters: HeightInMeters { _toFragment() }
        }

        /// AllAnimal.AsWarmBlooded.Height
        ///
        /// Parent Type: `Height`
        struct Height: TestSchema.SelectionSet {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }

          var feet: Int { __data["feet"] }
          var inches: Int? { __data["inches"] }
          var meters: Int { __data["meters"] }
        }
      }

      /// AllAnimal.AsPet
      ///
      /// Parent Type: `Pet`
      struct AsPet: TestSchema.InlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
        static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.Pet }
        static var __selections: [Apollo.Selection] { [
          .field("height", Height.self),
          .inlineFragment(AsWarmBlooded.self),
          .fragment(PetDetails.self),
        ] }

        var height: Height { __data["height"] }
        var id: TestSchema.ID { __data["id"] }
        var species: String { __data["species"] }
        var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        var predators: [Predator] { __data["predators"] }
        var humanName: String? { __data["humanName"] }
        var favoriteToy: String { __data["favoriteToy"] }
        var owner: Owner? { __data["owner"] }

        var asWarmBlooded: AsWarmBlooded? { _asInlineFragment() }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var petDetails: PetDetails { _toFragment() }
          var heightInMeters: HeightInMeters { _toFragment() }
        }

        /// AllAnimal.AsPet.Height
        ///
        /// Parent Type: `Height`
        struct Height: TestSchema.SelectionSet {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }
          static var __selections: [Apollo.Selection] { [
            .field("__typename", String.self),
            .field("relativeSize", GraphQLEnum<TestSchema.RelativeSize>.self),
            .field("centimeters", Double.self),
          ] }

          var relativeSize: GraphQLEnum<TestSchema.RelativeSize> { __data["relativeSize"] }
          var centimeters: Double { __data["centimeters"] }
          var feet: Int { __data["feet"] }
          var inches: Int? { __data["inches"] }
          var meters: Int { __data["meters"] }
        }

        typealias Owner = PetDetails.Owner

        /// AllAnimal.AsPet.AsWarmBlooded
        ///
        /// Parent Type: `WarmBlooded`
        struct AsWarmBlooded: TestSchema.InlineFragment {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
          static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.WarmBlooded }
          static var __selections: [Apollo.Selection] { [
            .fragment(WarmBloodedDetails.self),
          ] }

          var id: TestSchema.ID { __data["id"] }
          var height: Height { __data["height"] }
          var species: String { __data["species"] }
          var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
          var predators: [Predator] { __data["predators"] }
          var bodyTemperature: Int { __data["bodyTemperature"] }
          var humanName: String? { __data["humanName"] }
          var favoriteToy: String { __data["favoriteToy"] }
          var owner: Owner? { __data["owner"] }

          struct Fragments: FragmentContainer {
            let __data: DataDict
            init(_dataDict: DataDict) { __data = _dataDict }

            var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
            var heightInMeters: HeightInMeters { _toFragment() }
            var petDetails: PetDetails { _toFragment() }
          }

          /// AllAnimal.AsPet.AsWarmBlooded.Height
          ///
          /// Parent Type: `Height`
          struct Height: TestSchema.SelectionSet {
            let __data: DataDict
            init(_dataDict: DataDict) { __data = _dataDict }

            static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }

            var feet: Int { __data["feet"] }
            var inches: Int? { __data["inches"] }
            var meters: Int { __data["meters"] }
            var relativeSize: GraphQLEnum<TestSchema.RelativeSize> { __data["relativeSize"] }
            var centimeters: Double { __data["centimeters"] }
          }

          typealias Owner = PetDetails.Owner
        }
      }

      /// AllAnimal.AsCat
      ///
      /// Parent Type: `Cat`
      struct AsCat: TestSchema.InlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
        static var __parentType: any Apollo.ParentType { TestSchema.Objects.Cat }
        static var __selections: [Apollo.Selection] { [
          .field("isJellicle", Bool.self),
        ] }

        var isJellicle: Bool { __data["isJellicle"] }
        var id: TestSchema.ID { __data["id"] }
        var height: Height { __data["height"] }
        var species: String { __data["species"] }
        var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        var predators: [Predator] { __data["predators"] }
        var bodyTemperature: Int { __data["bodyTemperature"] }
        var humanName: String? { __data["humanName"] }
        var favoriteToy: String { __data["favoriteToy"] }
        var owner: Owner? { __data["owner"] }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var heightInMeters: HeightInMeters { _toFragment() }
          var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
          var petDetails: PetDetails { _toFragment() }
        }

        /// AllAnimal.AsCat.Height
        ///
        /// Parent Type: `Height`
        struct Height: TestSchema.SelectionSet {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }

          var feet: Int { __data["feet"] }
          var inches: Int? { __data["inches"] }
          var meters: Int { __data["meters"] }
          var relativeSize: GraphQLEnum<TestSchema.RelativeSize> { __data["relativeSize"] }
          var centimeters: Double { __data["centimeters"] }
        }

        typealias Owner = PetDetails.Owner
      }

      /// AllAnimal.AsClassroomPet
      ///
      /// Parent Type: `ClassroomPet`
      struct AsClassroomPet: TestSchema.InlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
        static var __parentType: any Apollo.ParentType { TestSchema.Unions.ClassroomPet }
        static var __selections: [Apollo.Selection] { [
          .inlineFragment(AsBird.self),
        ] }

        var id: TestSchema.ID { __data["id"] }
        var height: Height { __data["height"] }
        var species: String { __data["species"] }
        var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        var predators: [Predator] { __data["predators"] }

        var asBird: AsBird? { _asInlineFragment() }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var heightInMeters: HeightInMeters { _toFragment() }
        }

        /// AllAnimal.AsClassroomPet.Height
        ///
        /// Parent Type: `Height`
        struct Height: TestSchema.SelectionSet {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }

          var feet: Int { __data["feet"] }
          var inches: Int? { __data["inches"] }
          var meters: Int { __data["meters"] }
        }

        /// AllAnimal.AsClassroomPet.AsBird
        ///
        /// Parent Type: `Bird`
        struct AsBird: TestSchema.InlineFragment {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
          static var __parentType: any Apollo.ParentType { TestSchema.Objects.Bird }
          static var __selections: [Apollo.Selection] { [
            .field("wingspan", Double.self),
          ] }

          var wingspan: Double { __data["wingspan"] }
          var id: TestSchema.ID { __data["id"] }
          var height: Height { __data["height"] }
          var species: String { __data["species"] }
          var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
          var predators: [Predator] { __data["predators"] }
          var bodyTemperature: Int { __data["bodyTemperature"] }
          var humanName: String? { __data["humanName"] }
          var favoriteToy: String { __data["favoriteToy"] }
          var owner: Owner? { __data["owner"] }

          struct Fragments: FragmentContainer {
            let __data: DataDict
            init(_dataDict: DataDict) { __data = _dataDict }

            var heightInMeters: HeightInMeters { _toFragment() }
            var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
            var petDetails: PetDetails { _toFragment() }
          }

          /// AllAnimal.AsClassroomPet.AsBird.Height
          ///
          /// Parent Type: `Height`
          struct Height: TestSchema.SelectionSet {
            let __data: DataDict
            init(_dataDict: DataDict) { __data = _dataDict }

            static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }

            var feet: Int { __data["feet"] }
            var inches: Int? { __data["inches"] }
            var meters: Int { __data["meters"] }
            var relativeSize: GraphQLEnum<TestSchema.RelativeSize> { __data["relativeSize"] }
            var centimeters: Double { __data["centimeters"] }
          }

          typealias Owner = PetDetails.Owner
        }
      }

      /// AllAnimal.AsDog
      ///
      /// Parent Type: `Dog`
      struct AsDog: TestSchema.InlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
        static var __parentType: any Apollo.ParentType { TestSchema.Objects.Dog }
        static var __selections: [Apollo.Selection] { [
          .field("favoriteToy", String.self),
          .field("birthdate", TestSchema.CustomDate?.self),
        ] }

        var favoriteToy: String { __data["favoriteToy"] }
        var birthdate: TestSchema.CustomDate? { __data["birthdate"] }
        var id: TestSchema.ID { __data["id"] }
        var height: Height { __data["height"] }
        var species: String { __data["species"] }
        var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        var predators: [Predator] { __data["predators"] }
        var bodyTemperature: Int { __data["bodyTemperature"] }
        var humanName: String? { __data["humanName"] }
        var owner: Owner? { __data["owner"] }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var heightInMeters: HeightInMeters { _toFragment() }
          var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
          var petDetails: PetDetails { _toFragment() }
        }

        /// AllAnimal.AsDog.Height
        ///
        /// Parent Type: `Height`
        struct Height: TestSchema.SelectionSet {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          static var __parentType: any Apollo.ParentType { TestSchema.Objects.Height }

          var feet: Int { __data["feet"] }
          var inches: Int? { __data["inches"] }
          var meters: Int { __data["meters"] }
          var relativeSize: GraphQLEnum<TestSchema.RelativeSize> { __data["relativeSize"] }
          var centimeters: Double { __data["centimeters"] }
        }

        typealias Owner = PetDetails.Owner
      }
    }
  }
}
